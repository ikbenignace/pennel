import AppKit
import CoreAudio
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
    private(set) var detectedApp: String?
    private(set) var consentCopied = false
    static let consentSentence = "Ik neem notities met transcript via Musli."
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
        recorder.onSystemAudioSilent = { [weak self] in self?.systemAudioMissing = true }
    }

    var isRecording: Bool { phase == .recording }

    /// `detectedApp` is set when a detected call started this recording; only then does the call's end stop it.
    func start(title eventTitle: String? = nil, attendees: [String] = [], detectedApp: String? = nil) {
        guard phase != .recording else { return }
        defaultTitle = eventTitle ?? "Meeting \(Date().formatted(date: .omitted, time: .shortened))"
        title = defaultTitle
        notes = ""
        liveTranscript = []
        error = nil
        self.detectedApp = detectedApp
        do {
            let session = try state.engine.startMeetingWith(title: defaultTitle, attendees: attendees, keys: state.keys())
            let tapProcesses: (() -> [AudioObjectID])? = detectedApp.map { app in { MicUsageMonitor.processObjects(bundleId: app) } }
            try recorder.start(session: session, tapProcesses: tapProcesses, echoCancellation: state.settings.echoCancellation)
            consentCopied = false
            if state.settings.consentEnabled {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(Self.consentSentence, forType: .string)
                consentCopied = true
            }
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
        let template = state.templateId(state.settings.defaultTemplate)
        let wantedTitle = title.trimmingCharacters(in: .whitespaces)
        let renameTo = wantedTitle.isEmpty || wantedTitle == defaultTitle ? nil : wantedTitle
        let diarize = state.settings.diarization
        Task.detached(priority: .userInitiated) {
            try? session.stop()
            var id = session.id()
            if let renameTo, let renamed = try? engine.renameMeeting(id: id, title: renameTo) { id = renamed }
            if diarize, engine.modelState(id: engine.diarizerModelId()) == .downloaded {
                do {
                    _ = try Self.diarize(engine: engine, meetingId: id)
                } catch {
                    Logger(subsystem: "dev.musli.Musli", category: "meeting").error("diarization failed: \(String(describing: error), privacy: .public)")
                }
            }
            let summary = Result { try engine.summarizeMeeting(id: id, templateId: template, keys: keys) }
            await MainActor.run { self.finish(id: id, summary: summary) }
        }
    }

    /// Splits "Anderen" into voices from the saved others-track; failures leave the transcript as it was.
    nonisolated static func diarize(engine: MusliEngine, meetingId: String) throws -> UInt32 {
        let track = URL(fileURLWithPath: engine.meetingDir(id: meetingId)).appendingPathComponent("anderen.m4a")
        let samples = try AudioDecoder.monoSamples(of: track)
        return try engine.diarizeMeeting(id: meetingId, samples: samples)
    }

    private func finish(id: String, summary: Result<String, Error>) {
        // A back-to-back call may already be recording; only the library needs to learn about this one.
        guard phase == .summarizing else {
            onLibraryChanged?()
            return
        }
        meetingId = id
        detectedApp = nil
        session = nil
        phase = .finished
        if case .failure(let failure) = summary, (failure as? MusliError) != .NoSpeech {
            error = "Samenvatten mislukt: \(failure)"
        }
        onLibraryChanged?()
    }

    /// Silences the user's microphone in the meeting while they dictate into another app.
    func setDictating(_ dictating: Bool) {
        guard phase == .recording else { return }
        session?.setDictating(dictating: dictating)
    }

    /// Quitting mid-meeting: close the audio files and transcribe what's left, but skip the summary.
    func stopForQuit() async {
        guard phase == .recording, let session else { return }
        ticker?.invalidate()
        recorder.stop()
        saveNotesNow()
        await Task.detached { try? session.stop() }.value
        self.session = nil
        phase = .finished
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
