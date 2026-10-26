import AVFoundation
import os
import PennelMobileKit
import SwiftUI
import UIKit

@MainActor
@Observable
final class DictationController {
    enum Phase { case idle, listening, processing }

    private(set) var phase = Phase.idle
    private(set) var history: [HistoryEntry] = []
    private(set) var retrying: Set<String> = []
    var notice: String?
    var noticeIsError = false

    private let model: AppModel
    @ObservationIgnored private let capture = SharedMicrophone.shared.client()
    @ObservationIgnored private let audioSession = SharedAudioSession.shared.client(.dictation)
    @ObservationIgnored private var meetingDictationStartMs: UInt64?
    private let buffer = SampleBuffer()
    private var startedAt: Date?
    @ObservationIgnored private var observers: [NSObjectProtocol] = []
    private static let log = Logger(subsystem: "app.pennel.ios", category: "dictation")

    init(model: AppModel) {
        self.model = model
        observeInterruptions()
    }

    /// A call or a lost microphone ends the recording; what was said so far is still transcribed.
    private func observeInterruptions() {
        let center = NotificationCenter.default
        observers.append(center.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { [weak self] note in
            guard let raw = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt,
                  AVAudioSession.InterruptionType(rawValue: raw) == .began
            else { return }
            MainActor.assumeIsolated { self?.interrupted() }
        })
        observers.append(center.addObserver(forName: AVAudioSession.routeChangeNotification, object: nil, queue: .main) { [weak self] note in
            guard let raw = note.userInfo?[AVAudioSessionRouteChangeReasonKey] as? UInt,
                  AVAudioSession.RouteChangeReason(rawValue: raw) == .oldDeviceUnavailable
            else { return }
            MainActor.assumeIsolated { self?.interrupted() }
        })
        observers.append(center.addObserver(forName: AVAudioSession.mediaServicesWereResetNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.interrupted() }
        })
    }

    private func interrupted() {
        guard phase == .listening else { return }
        Task { await stop() }
    }

    func toggle() {
        switch phase {
        case .idle: Task { await start() }
        case .listening: Task { await stop() }
        case .processing: break
        }
    }

    /// A Shortcut or the Action Button asks for a dictation; one already running keeps going.
    func startIfIdle() {
        guard phase == .idle else { return }
        Task { await start() }
    }

    func loadHistory() {
        let engine = model.engine
        Task {
            history = await Task.detached { (try? engine.history(limit: 200)) ?? [] }.value
        }
    }

    func copy(_ text: String) {
        UIPasteboard.general.string = text
        show("Gekopieerd", error: false)
    }

    func retry(_ entry: HistoryEntry) {
        retrying.insert(entry.id)
        let engine = model.engine
        Task {
            let outcome = await Task.detached { () -> Result<Void, Error> in
                do {
                    if entry.status == .unsaved { try engine.saveHistory(id: entry.id) }
                    else { _ = try engine.retry(historyId: entry.id, keys: Keychain.apiKeys()) }
                    return .success(())
                } catch { return .failure(error) }
            }.value
            retrying.remove(entry.id)
            switch outcome {
            case .success:
                show(entry.status == .unsaved ? "Dictaat bewaard." : "Dictaat hersteld. Je kunt de tekst nu kopiëren.", error: false)
            case .failure(let error):
                let action = entry.status == .unsaved ? "Bewaren" : "Opnieuw proberen"
                show("\(action) mislukt: \(ErrorText.message(error))", error: true)
            }
            loadHistory()
        }
    }

    private func start() async {
        guard await MicCapture.requestPermission() else {
            show("Geen toegang tot de microfoon. Sta de microfoon toe in Instellingen.", error: true)
            return
        }
        guard phase == .idle else { return }
        notice = nil
        buffer.reset()
        // A recording meeting shares this microphone: it keeps the time but not the dictated words (spec §5.3).
        let meeting = MeetingRuntime.shared
        if meeting.isActive {
            meeting.setDictating(true)
            meetingDictationStartMs = meeting.elapsedMs
        }
        do {
            try audioSession.activate()
            let buffer = buffer
            try capture.start { buffer.append($0) }
        } catch {
            Self.log.error("capture start: \(ErrorText.code(error), privacy: .public)")
            capture.stop()
            audioSession.deactivate()
            endMeetingDictation()
            show("Mislukt", error: true)
            return
        }
        startedAt = Date()
        phase = .listening
    }

    private func stop() async {
        guard phase == .listening else { return }
        capture.stop()
        audioSession.deactivate()
        endMeetingDictation()
        let samples = buffer.take()
        let seconds = Date().timeIntervalSince(startedAt ?? Date())
        phase = .processing
        let result = await DictationRunner(engine: model.engine).run(samples: samples)
        phase = .idle
        switch result {
        case .success(let dictation):
            Self.log.info("dictated \(Int(seconds), privacy: .public)s, \(dictation.text.count, privacy: .public) chars")
            copy(dictation.text)
        case .failure(let error):
            Self.log.error("dictation failed: \(ErrorText.code(error), privacy: .public)")
            show(ErrorText.message(error), error: true)
        }
        loadHistory()
    }

    private func endMeetingDictation() {
        guard let start = meetingDictationStartMs else { return }
        meetingDictationStartMs = nil
        let meeting = MeetingRuntime.shared
        guard meeting.isActive else { return }
        meeting.setDictating(false)
        meeting.markDictation(startMs: start, endMs: meeting.elapsedMs)
    }

    private func show(_ text: String, error: Bool) {
        notice = text
        noticeIsError = error
        let shown = text
        Task {
            try? await Task.sleep(for: .seconds(error ? 6 : 2))
            if notice == shown { notice = nil }
        }
    }
}

/// The audio callback runs on a realtime thread, so the samples live behind a lock instead of the main actor.
private final class SampleBuffer: @unchecked Sendable {
    private let lock = NSLock()
    private var samples: [Float] = []

    func append(_ chunk: [Float]) { lock.withLock { samples += chunk } }
    func reset() { lock.withLock { samples = [] } }
    func take() -> [Float] { lock.withLock { defer { samples = [] }; return samples } }
}

struct DictationView: View {
    @Environment(AppModel.self) private var model
    @State private var controller: DictationController?
    private let router = AppRouter.shared

    var body: some View {
        NavigationStack {
            Group {
                if let controller {
                    content(controller)
                } else {
                    Color.clear
                }
            }
            .navigationTitle(Copy.Tab.dictate)
        }
        .task {
            if controller == nil { controller = DictationController(model: model) }
            controller?.loadHistory()
            startRequestedDictation()
        }
        .onChange(of: router.dictationRequests) { startRequestedDictation() }
    }

    private func startRequestedDictation() {
        guard router.takeDictationRequest() else { return }
        controller?.startIfIdle()
    }

    private func content(_ controller: DictationController) -> some View {
        List {
            Section(Copy.Dictation.sessionCard) {
                SessionCard(session: DictationSession.shared)
            }
            .pennelRow()
            Section {
                VStack(spacing: 12) {
                    Button(action: controller.toggle) {
                        Text(buttonTitle(controller.phase))
                            .font(Theme.font(.headline, weight: .semibold))
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 8)
                    }
                    .buttonStyle(.borderedProminent)
                    .controlSize(.large)
                    .disabled(controller.phase == .processing)
                    if controller.phase == .processing {
                        Text(Copy.Dictation.processing).font(Theme.font(.footnote)).foregroundStyle(Brand.muted.color)
                    }
                    if let notice = controller.notice {
                        Text(notice)
                            .font(Theme.font(.footnote))
                            .foregroundStyle(controller.noticeIsError ? Color.red : Brand.muted.color)
                            .accessibilityAddTraits(.updatesFrequently)
                    }
                }
                .listRowBackground(Color.clear)
            }
            Section(Copy.Dictation.history) {
                if controller.history.isEmpty {
                    Text(Copy.Dictation.historyEmpty).foregroundStyle(Brand.muted.color).pennelRow()
                }
                ForEach(controller.history, id: \.id) { entry in
                    HistoryRow(entry: entry, retrying: controller.retrying.contains(entry.id), controller: controller)
                        .pennelRow()
                }
            }
        }
        .pennelList()
    }

    private func buttonTitle(_ phase: DictationController.Phase) -> String {
        switch phase {
        case .idle: Copy.Dictation.dictateToClipboard
        case .listening: Copy.Dictation.listening
        case .processing: "Bezig…"
        }
    }
}

private struct HistoryRow: View {
    let entry: HistoryEntry
    let retrying: Bool
    let controller: DictationController

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if entry.status == .unsaved {
                Text("Nog niet bewaard").font(Theme.font(.footnote)).foregroundStyle(.red)
            }
            if entry.status == .failed {
                Text("Mislukt: \(entry.error ?? "onbekende fout")").foregroundStyle(.red)
            } else {
                Text(entry.text)
            }
            Text(Date(timeIntervalSince1970: Double(entry.createdAtMs) / 1000).formatted(.dateTime.locale(Locale(identifier: "nl-BE")).day().month(.abbreviated).year().hour().minute()))
                .font(Theme.font(.footnote))
                .foregroundStyle(Brand.muted.color)
            HStack {
                if entry.status != .failed {
                    Button(Copy.Dictation.copy) { controller.copy(entry.text) }
                        .accessibilityLabel("Dictaat kopiëren")
                }
                if entry.status == .unsaved || (entry.status == .failed && entry.hasAudio) {
                    Button(retrying ? "Bezig…" : (entry.status == .unsaved ? "Opnieuw bewaren" : Copy.Dictation.retry)) {
                        controller.retry(entry)
                    }
                    .disabled(retrying)
                }
            }
            .buttonStyle(.borderless)
            .font(Theme.font(.subheadline))
        }
    }
}

/// Spec §4.2: the keyboard session at the top of the tab, in its three states.
private struct SessionCard: View {
    let session: DictationSession

    var body: some View {
        TimelineView(.periodic(from: .now, by: 1)) { context in
            VStack(alignment: .leading, spacing: Brand.Spacing.s3) {
                HStack {
                    Text(status(now: context.date))
                        .font(Theme.font(.headline, weight: .semibold))
                    Spacer()
                    switch session.phase {
                    case .off:
                        Button(Copy.Dictation.startSession) { Task { await session.start() } }
                            .buttonStyle(.bordered)
                    case .ready:
                        Button(Copy.Dictation.stopSession) { session.stop() }
                            .buttonStyle(.bordered)
                    case .listening, .processing:
                        EmptyView()
                    }
                }
                if session.phase == .ready {
                    Text(Copy.Dictation.sessionReadyExplanation)
                        .font(Theme.font(.footnote))
                        .foregroundStyle(Brand.muted.color)
                }
                if session.phase == .off, let error = session.startError {
                    Text(error).font(Theme.font(.footnote)).foregroundStyle(.red)
                }
            }
            .padding(.vertical, Brand.Spacing.s1)
        }
    }

    private func status(now: Date) -> String {
        switch session.phase {
        case .off: Copy.Dictation.sessionOff
        case .ready: Copy.Dictation.sessionReady(remaining: max(0, (session.expiresAt ?? now).timeIntervalSince(now)))
        case .listening: Copy.Dictation.listening
        case .processing: Copy.Dictation.processing
        }
    }
}
