import AppKit
@preconcurrency import PennelCore
import PennelKit

@MainActor
final class DictationController {
    private static let noticeDuration: Duration = .seconds(2)

    private let state: AppState
    private let pill = PillPanel()
    private let recorder = AudioRecorder()
    private let queue = PasteQueue<String>()
    private let log = Diagnostics("dictation")
    private var gesture = HotkeyGesture()
    private var command = CommandIntent()
    private var status = DictationStatus()
    private var monitor: HotkeyMonitor?
    private var ticker: Timer?
    private var noticeTask: Task<Void, Never>?
    private var recordingApp: String?
    private var recordingPid: pid_t?
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
        let settings = state.settings
        log.notice(
            "sneltoets actief=\(started) toegankelijkheid=\(Permissions.accessibilityTrusted) · dicteren \(settings.dictationStt.providerId)/\(settings.dictationStt.model) · opkuis \(settings.llm.providerId)/\(settings.llm.model) · taal \(settings.language ?? "automatisch") · modus \(settings.mode)"
        )
        return started
    }

    func setTrigger(_ trigger: HotkeyTrigger) {
        monitor?.setTrigger(trigger)
        gesture = HotkeyGesture()
    }

    private func handle(_ edge: KeyEdge, at time: TimeInterval) {
        log.debug("hotkey \(String(describing: edge))")
        perform(gesture.handle(edge, at: time))
    }

    private func interrupt() {
        perform(gesture.interrupt())
    }

    private func shift(_ down: Bool) {
        let wasCommand = command.isCommand
        command.shift(down, keyHeld: isRecording && (monitor?.isKeyDown ?? false))
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
        recordingPid = NSWorkspace.shared.frontmostApplication?.processIdentifier
        do {
            try recorder.start { [weak self] level in
                Task { @MainActor in self?.pill.model.level = level }
            }
        } catch is NoInputDevice {
            notify("Geen microfoon gevonden")
            return
        } catch {
            log.error("opnemen starten mislukt: \(error.localizedDescription)")
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
        if SilenceWatch.isDigitalSilence(samples) {
            log.error("microfoon leverde enkel stilte (exacte nullen) — invoerapparaat of voice processing")
            command.reset()
            status.recordingCancelled()
            notify("Microfoon geeft geen geluid — kies een ander invoerapparaat of herstart Pennel")
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
            let pid = recordingPid
            let pendingPastes = insertion
            Task {
                // Earlier pastes restore the clipboard first, so ⌘C never snapshots Pennel's own text.
                await pendingPastes?.value
                guard NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else {
                    self.finish(seq, .failure(CommandError.appChanged), released: released, isCommand: true)
                    return
                }
                let selection = await SelectionReader.read()
                let outcome = await Task.detached(priority: .userInitiated) {
                    Result { try engine.command(request: request, selection: selection) }
                }.value
                let stillThere = NSWorkspace.shared.frontmostApplication?.processIdentifier == pid
                self.finish(seq, stillThere ? outcome : .failure(CommandError.appChanged), released: released, isCommand: true)
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
        let kind = isCommand ? "opdracht" : "dictaat"
        let elapsed = released.duration(to: .now).formatted(.units(allowed: [.milliseconds]))
        switch outcome {
        case .success(let result):
            text = result.text
            notice = Self.notice(for: result.cleanup)
            let line = "\(kind) klaar in \(elapsed): \(result.raw.count) → \(result.text.count) tekens, opkuis \(Self.describe(result.cleanup))"
            if case .failed = result.cleanup { log.error(line) } else { log.notice(line) }
        case .failure(let error):
            text = nil
            notice = isCommand ? Self.commandNotice(for: error) : Self.notice(for: error)
            if (error as? PennelError) == .NoSpeech {
                log.notice("\(kind): geen spraak herkend (\(elapsed))")
            } else {
                log.error("\(kind) mislukt na \(elapsed): \(type(of: error))")
            }
        }
        status.dictationFinished(notice: notice)
        if notice != nil { scheduleNoticeExpiry() }
        render()
        NotificationCenter.default.post(name: .pennelHistoryChanged, object: nil)

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

    private static func describe(_ cleanup: CleanupStatus) -> String {
        switch cleanup {
        case .applied: "toegepast"
        case .skipped: "overgeslagen (snelle modus)"
        case .rejected: "afgewezen (model antwoordde in plaats van op te kuisen)"
        case .failed: "mislukt"
        }
    }

    private static func notice(for cleanup: CleanupStatus) -> String? {
        switch cleanup {
        case .failed(let message) where message.lowercased().contains("timed out") || message.lowercased().contains("timeout"):
            "Opkuisen duurde te lang — ruwe tekst geplakt (zie logboek)"
        case .failed: "Opkuisen mislukt — ruwe tekst geplakt (zie logboek)"
        case .rejected: "Model antwoordde i.p.v. op te kuisen — ruwe tekst geplakt"
        default: nil
        }
    }

    private enum CommandError: Error { case appChanged }

    /// Commands are not kept in history when they fail, so these notices don't promise a retry.
    private static func commandNotice(for error: Error) -> String? {
        if case .appChanged = error as? CommandError { return "Andere app actief — opdracht niet geplakt" }
        switch error as? PennelError {
        case .NoSpeech: return nil
        case .MissingApiKey: return "Geen API-sleutel ingesteld"
        case .NotFound: return "Geen taalmodel ingesteld"
        case .Http(let status, _) where status == 401 || status == 403: return "API-sleutel geweigerd"
        case .InvalidResponse(let message) where message == "selection too long": return "Selectie te lang voor een opdracht"
        case .Network: return "Geen verbinding"
        default: return "Opdracht mislukt"
        }
    }

    private static func notice(for error: Error) -> String? {
        switch error as? PennelError {
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
