import AppKit
@preconcurrency import MusliCore
import MusliKit
import os

@MainActor
final class DictationController {
    private static let noticeDuration: Duration = .seconds(2)

    private let state: AppState
    private let pill = PillPanel()
    private let recorder = AudioRecorder()
    private let queue = PasteQueue<String>()
    private let log = Logger(subsystem: "dev.musli.Musli", category: "dictation")
    private var gesture = HotkeyGesture()
    private var command = CommandIntent()
    private var status = DictationStatus()
    private var monitor: HotkeyMonitor?
    private var ticker: Timer?
    private var noticeTask: Task<Void, Never>?
    private var recordingApp: String?
    private var isRecording = false
    private var insertion: Task<Void, Never>?
    weak var meeting: MeetingController?

    init(state: AppState) {
        self.state = state
    }

    var isListening: Bool { monitor?.isRunning ?? false }

    /// Starts the hotkey tap; returns false while Accessibility/Input Monitoring is not granted yet.
    func start() -> Bool {
        if monitor == nil {
            monitor = HotkeyMonitor(
                trigger: state.settings.hotkey,
                onEdge: { [weak self] edge, time in
                    MainActor.assumeIsolated { self?.handle(edge, at: time) }
                },
                onInterrupt: { [weak self] in
                    MainActor.assumeIsolated { self?.interrupt() }
                },
                onShift: { [weak self] down in
                    MainActor.assumeIsolated { self?.shift(down) }
                }
            )
        }
        let started = monitor?.start() ?? false
        log.notice("hotkey tap started=\(started, privacy: .public) accessibility=\(Permissions.accessibilityTrusted, privacy: .public)")
        return started
    }

    func setTrigger(_ trigger: HotkeyTrigger) {
        monitor?.setTrigger(trigger)
        gesture = HotkeyGesture()
    }

    private func handle(_ edge: KeyEdge, at time: TimeInterval) {
        log.debug("hotkey \(String(describing: edge), privacy: .public)")
        perform(gesture.handle(edge, at: time))
    }

    private func interrupt() {
        perform(gesture.interrupt())
    }

    private func shift(_ down: Bool) {
        let wasCommand = command.isCommand
        command.shift(down, keyHeld: isRecording)
        if command.isCommand && !wasCommand {
            status.markCommand()
            render()
        }
    }

    private func perform(_ actions: [GestureAction]) {
        for action in actions {
            switch action {
            case .start: beginRecording()
            case .cancel: cancelRecording()
            case .submit: submitRecording()
            }
        }
    }

    private func beginRecording() {
        guard Permissions.microphone == .authorized else {
            notify("Geen toegang tot de microfoon")
            return
        }
        recordingApp = NSWorkspace.shared.frontmostApplication?.bundleIdentifier
        do {
            try recorder.start { [weak self] level in
                Task { @MainActor in self?.pill.model.level = level }
            }
        } catch is NoInputDevice {
            notify("Geen microfoon gevonden")
            return
        } catch {
            log.error("recorder start failed: \(error.localizedDescription)")
            notify("Opnemen lukt niet")
            return
        }
        isRecording = true
        meeting?.setDictating(true)
        status.recordingStarted(handsFree: gesture.isHandsFree)
        command.reset()
        command.shift(NSEvent.modifierFlags.contains(.shift), keyHeld: true)
        if command.isCommand { status.markCommand() }
        render()
        ticker = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self else { return }
                self.perform(self.gesture.tick(at: ProcessInfo.processInfo.systemUptime))
            }
        }
    }

    private func stopRecording() -> [Float] {
        ticker?.invalidate()
        ticker = nil
        guard isRecording else { return [] }
        isRecording = false
        meeting?.setDictating(false)
        return recorder.stop()
    }

    private func cancelRecording() {
        _ = stopRecording()
        command.reset()
        status.recordingCancelled()
        render()
    }

    private func submitRecording() {
        let samples = stopRecording()
        guard !samples.isEmpty else {
            status.recordingCancelled()
            render()
            return
        }
        let seq = queue.reserve()
        let request = DictationRequest(
            samples: samples,
            sampleRate: UInt32(AudioRecorder.sampleRate),
            appBundleId: recordingApp,
            keys: state.keys()
        )
        let engine = state.engine
        let released = ContinuousClock.now
        let isCommand = command.isCommand
        command.reset()
        status.recordingSubmitted()
        render()
        if isCommand {
            Task {
                let selection = await SelectionReader.read()
                let outcome = await Task.detached(priority: .userInitiated) {
                    Result { try engine.command(request: request, selection: selection) }
                }.value
                self.finish(seq, outcome, released: released, isCommand: true)
            }
            return
        }
        Task.detached(priority: .userInitiated) {
            let outcome = Result { try engine.dictate(request: request) }
            await MainActor.run { self.finish(seq, outcome, released: released) }
        }
    }

    private func finish(
        _ seq: Int,
        _ outcome: Result<DictationResult, Error>,
        released: ContinuousClock.Instant,
        isCommand: Bool = false
    ) {
        let text: String?
        let notice: String?
        switch outcome {
        case .success(let result):
            text = result.text
            notice = Self.notice(for: result.cleanup)
        case .failure(let error):
            text = nil
            notice = isCommand ? Self.commandNotice(for: error) : Self.notice(for: error)
        }
        status.dictationFinished(notice: notice)
        if notice != nil { scheduleNoticeExpiry() }
        render()
        NotificationCenter.default.post(name: .musliHistoryChanged, object: nil)

        let ready = queue.fulfill(seq, with: text)
        let previous = insertion
        insertion = Task {
            await previous?.value
            for text in ready {
                await TextInserter.insert(text)
            }
            if !ready.isEmpty {
                log.info("release→paste \(released.duration(to: .now).formatted(.units(allowed: [.milliseconds])))")
            }
        }
    }

    private func notify(_ message: String) {
        status.showNotice(message)
        scheduleNoticeExpiry()
        render()
    }

    private func scheduleNoticeExpiry() {
        noticeTask?.cancel()
        noticeTask = Task { [weak self] in
            try? await Task.sleep(for: Self.noticeDuration)
            guard !Task.isCancelled, let self else { return }
            self.status.noticeExpired()
            self.render()
        }
    }

    private func render() {
        pill.show(status.pill)
    }

    private static func notice(for cleanup: CleanupStatus) -> String? {
        switch cleanup {
        case .failed: "Opkuisen mislukt — ruwe tekst geplakt"
        case .rejected: "Model antwoordde i.p.v. op te kuisen — ruwe tekst geplakt"
        default: nil
        }
    }

    /// Commands are not kept in history when they fail, so these notices don't promise a retry.
    private static func commandNotice(for error: Error) -> String? {
        switch error as? MusliError {
        case .NoSpeech: nil
        case .MissingApiKey: "Geen API-sleutel ingesteld"
        case .InvalidResponse(let message) where message == "selection too long": "Selectie te lang voor een opdracht"
        case .Network: "Geen verbinding"
        default: "Opdracht mislukt"
        }
    }

    private static func notice(for error: Error) -> String? {
        switch error as? MusliError {
        case .NoSpeech: nil
        case .MissingApiKey: "Geen API-sleutel ingesteld"
        case .ModelNotDownloaded: "Model nog niet gedownload — bewaard in geschiedenis"
        case .Http(let status, _) where status == 401 || status == 403: "API-sleutel geweigerd — bewaard in geschiedenis"
        case .Network: "Geen verbinding — bewaard in geschiedenis"
        case .Http, .InvalidResponse: "Mislukt — bewaard in geschiedenis"
        default: "Mislukt"
        }
    }
}
