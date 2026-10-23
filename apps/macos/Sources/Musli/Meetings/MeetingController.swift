import Foundation
@preconcurrency import MusliCore
import Observation
import os

@MainActor
@Observable
final class MeetingController {
    enum Phase: Equatable {
        case idle
        case recording
        case summarizing
        case finished
    }

    private(set) var phase: Phase = .idle
    private(set) var meetingId: String?
    private(set) var elapsed: TimeInterval = 0
    private(set) var liveTranscript: [Segment] = []
    private(set) var systemAudioMissing = false
    private(set) var error: String?
    var title = ""
    var notes = "" {
        didSet { scheduleNotesSave() }
    }
    var onLibraryChanged: (() -> Void)?

    private let state: AppState
    private let recorder = MeetingRecorder()
    private let log = Logger(subsystem: "dev.musli.Musli", category: "meeting")
    private var session: MeetingSession?
    private var defaultTitle = ""
    private var ticker: Timer?
    private var notesTask: Task<Void, Never>?

    init(state: AppState) {
        self.state = state
    }

    var isRecording: Bool { phase == .recording }

    func start() {
        guard phase != .recording, phase != .summarizing else { return }
        defaultTitle = "Meeting \(Date().formatted(date: .omitted, time: .shortened))"
        title = defaultTitle
        notes = ""
        liveTranscript = []
        error = nil
        do {
            let session = try state.engine.startMeeting(title: defaultTitle, keys: state.keys())
            try recorder.start(session: session)
            self.session = session
            meetingId = session.id()
            systemAudioMissing = !recorder.systemAudioAvailable
            phase = .recording
            startTicker()
            onLibraryChanged?()
        } catch {
            log.error("meeting start failed: \(String(describing: error), privacy: .public)")
            self.error = error is NoInputDevice ? "Geen microfoon gevonden" : "Opnemen lukt niet: \(error)"
            recorder.stop()
            session = nil
        }
    }

    func stop() {
        guard phase == .recording, let session else { return }
        ticker?.invalidate()
        recorder.stop()
        saveNotesNow()
        phase = .summarizing
        let engine = state.engine
        let keys = state.keys()
        let template = state.settings.defaultTemplate
        let wantedTitle = title.trimmingCharacters(in: .whitespaces)
        let renameTo = wantedTitle.isEmpty || wantedTitle == defaultTitle ? nil : wantedTitle
        Task.detached(priority: .userInitiated) {
            try? session.stop()
            var id = session.id()
            if let renameTo, let renamed = try? engine.renameMeeting(id: id, title: renameTo) { id = renamed }
            let summary = Result { try engine.summarizeMeeting(id: id, templateId: template, keys: keys) }
            await MainActor.run { self.finish(id: id, summary: summary) }
        }
    }

    private func finish(id: String, summary: Result<String, Error>) {
        meetingId = id
        session = nil
        phase = .finished
        if case .failure(let failure) = summary, (failure as? MusliError) != .NoSpeech {
            error = "Samenvatten mislukt: \(failure)"
        }
        onLibraryChanged?()
    }

    func reset() {
        guard phase == .finished else { return }
        phase = .idle
    }

    private func startTicker() {
        var tick = 0
        ticker = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self else { return }
                self.elapsed = self.recorder.elapsed
                tick += 1
                if tick % 2 == 0, let session = self.session { self.liveTranscript = session.transcript() }
            }
        }
    }

    private func scheduleNotesSave() {
        guard phase == .recording else { return }
        notesTask?.cancel()
        notesTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(1))
            guard !Task.isCancelled else { return }
            self?.saveNotesNow()
        }
    }

    private func saveNotesNow() {
        notesTask?.cancel()
        try? session?.setNotes(text: notes)
    }
}
