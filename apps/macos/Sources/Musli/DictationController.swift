import AppKit
@preconcurrency import MusliCore
import MusliKit
import os


@MainActor
final class DictationController {
    private let state: AppState
    private let pill = PillPanel()
    private let recorder = AudioRecorder()
    private let queue = PasteQueue<String>()
    private let log = Logger(subsystem: "dev.musli.Musli", category: "dictation")
    private var gesture = HotkeyGesture()
    private var monitor: HotkeyMonitor?
    private var ticker: Timer?
    private var recordingApp: String?
    private var isRecording = false
    private var inFlight = 0
    private var insertion: Task<Void, Never>?

    init(state: AppState) {
        self.state = state
    }

    var isListening: Bool { monitor?.isRunning ?? false }

    /// Starts the hotkey tap; returns false while Accessibility/Input Monitoring is not granted yet.
    func start() -> Bool {
        if monitor == nil {
            monitor = HotkeyMonitor(trigger: state.settings.hotkey) { [weak self] edge, time in
                MainActor.assumeIsolated { self?.handle(edge, at: time) }
            }
        }
        return monitor?.start() ?? false
    }

    func setTrigger(_ trigger: HotkeyTrigger) {
        monitor?.setTrigger(trigger)
        gesture = HotkeyGesture()
    }

    private func handle(_ edge: KeyEdge, at time: TimeInterval) {
        perform(gesture.handle(edge, at: time))
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
            pill.show(.error("Geen toegang tot de microfoon"))
            return
        }
        recordingApp = NSWorkspace.shared.frontmostApplication?.bundleIdentifier
        do {
            try recorder.start { [weak self] level in
                Task { @MainActor in self?.pill.model.level = level }
            }
        } catch {
            log.error("recorder start failed: \(error.localizedDescription)")
            pill.show(.error("Opnemen lukt niet"))
            return
        }
        isRecording = true
        pill.show(gesture.isHandsFree ? .handsFree : .listening)
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
        return recorder.stop()
    }

    private func cancelRecording() {
        _ = stopRecording()
        refreshPill()
    }

    private func submitRecording() {
        let samples = stopRecording()
        guard !samples.isEmpty else {
            refreshPill()
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
        inFlight += 1
        refreshPill()
        Task.detached(priority: .userInitiated) {
            let outcome = Result { try engine.dictate(request: request) }
            await MainActor.run { self.finish(seq, outcome, released: released) }
        }
    }

    private func finish(_ seq: Int, _ outcome: Result<DictationResult, Error>, released: ContinuousClock.Instant) {
        inFlight -= 1
        let text: String?
        switch outcome {
        case .success(let result):
            text = result.text
            if case .failed(let message) = result.cleanup {
                log.notice("cleanup failed, pasted raw text: \(message)")
            }
        case .failure(let error):
            text = nil
            if let message = Self.message(for: error) {
                pill.show(.error(message))
            }
        }
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
        if case .success = outcome { refreshPill() }
    }

    private func refreshPill() {
        if isRecording {
            pill.show(gesture.isHandsFree ? .handsFree : .listening)
        } else if inFlight > 0 {
            pill.show(.working)
        } else if case .error = pill.model.state {
            return
        } else {
            pill.show(.hidden)
        }
    }

    private static func message(for error: Error) -> String? {
        switch error as? MusliError {
        case .NoSpeech: nil
        case .MissingApiKey: "Geen API-sleutel ingesteld"
        case .Http(let status, _) where status == 401 || status == 403: "API-sleutel geweigerd"
        case .Network: "Geen verbinding — bewaard in geschiedenis"
        default: "Mislukt — bewaard in geschiedenis"
        }
    }
}
