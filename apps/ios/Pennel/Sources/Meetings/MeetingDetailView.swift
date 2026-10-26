import AVFoundation
import os
import PennelMobileKit
import SwiftUI

/// A finished meeting: **Notities**, **Transcript**, **Vragen**, as the desktop meeting view.
struct MeetingDetailView: View {
    @Environment(AppModel.self) private var model
    let id: String
    var jumpMs: UInt64?

    private enum Tab: Hashable { case notes, transcript, questions }

    @State private var runtime = MeetingRuntime.shared
    @State private var info: MeetingInfo?
    @State private var loaded = false
    @State private var notes = ""
    @State private var savedNotes = ""
    @State private var summaryLines: [NoteLine] = []
    @State private var transcript: [Segment] = []
    @State private var tab = Tab.notes
    @State private var template = ""
    @State private var working = false
    @State private var message = ""
    @State private var saveNotes: Task<Void, Never>?
    @State private var player = MeetingPlayer()
    @State private var conflicts: [String] = []

    private static let log = Logger(subsystem: "app.pennel.ios", category: "meeting")

    private var busy: Bool { runtime.isRecording || runtime.phase == .finishing }
    private var canSummarize: Bool { MeetingRuntime.languageModelReady(model.settings) }
    private var meetingDirectory: URL { URL(fileURLWithPath: model.engine.meetingDir(id: id)) }
    private var audioURL: URL {
        URL(fileURLWithPath: model.engine.meetingDir(id: id)).appendingPathComponent(AACArchive.fileName)
    }

    var body: some View {
        Group {
            if let info {
                content(info)
            } else if loaded {
                Text(Copy.Meetings.notFound).foregroundStyle(Brand.muted.color)
            } else {
                ProgressView()
            }
        }
        .background(Brand.surface.color)
        .navigationBarTitleDisplayMode(.inline)
        .task { await load() }
        .onDisappear {
            player.stop()
            flushNotes()
        }
    }

    private func content(_ info: MeetingInfo) -> some View {
        VStack(spacing: 0) {
            VStack(alignment: .leading, spacing: Brand.Spacing.s2) {
                Text(info.title).font(.instrumentSerif(Brand.FontSize.displayTitle))
                Text([MeetingDates.dateTimeLabel(info.startIso),
                      info.durationMin > 0 ? "\(info.durationMin) min" : ""]
                    .filter { !$0.isEmpty }.joined(separator: " · "))
                    .font(Theme.font(.footnote))
                    .foregroundStyle(Brand.muted.color)
                if let record = runtime.interruption(for: id) {
                    InterruptionNotice(record: record)
                }
                ForEach(conflicts, id: \.self) { name in
                    ConflictBanner(name: name, directory: meetingDirectory, removable: runtime.activeMeetingId != id) {
                        loadConflicts()
                        CloudSyncRunner.shared.poke()
                    }
                }
                Picker("Meeting", selection: $tab) {
                    Text(Copy.Meetings.notes).tag(Tab.notes)
                    Text(Copy.Meetings.transcript).tag(Tab.transcript)
                    Text(Copy.Meetings.questions).tag(Tab.questions)
                }
                .pickerStyle(.segmented)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, Brand.Spacing.s5)
            .padding(.vertical, Brand.Spacing.s3)
            if tab == .questions {
                AskChat(meetingId: id)
            } else {
                List {
                    if tab == .notes { notesSection } else { transcriptSection }
                    actions(info)
                }
                .pennelList()
            }
        }
    }

    // MARK: Notities

    @ViewBuilder private var notesSection: some View {
        Section {
            if summaryLines.isEmpty {
                if notes.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    Text(Copy.Meetings.noSummary).foregroundStyle(Brand.muted.color)
                } else {
                    MarkdownText(notes)
                }
            } else {
                VStack(alignment: .leading, spacing: Brand.Spacing.s1) {
                    ForEach(Array(summaryLines.enumerated()), id: \.offset) { _, line in
                        MarkdownText(line.text)
                            .foregroundStyle(line.mine ? Brand.foreground.color : Brand.muted.color)
                    }
                }
            }
        }
        .pennelRow()
        Section {
            DisclosureGroup(Copy.Meetings.yourNotes) {
                TextEditor(text: $notes)
                    .frame(minHeight: 160)
                    .scrollContentBackground(.hidden)
                    .disabled(busy && runtime.activeMeetingId == id)
                    .onChange(of: notes) { scheduleNotesSave() }
            }
        }
        .pennelRow()
    }

    // MARK: Transcript

    @ViewBuilder private var transcriptSection: some View {
        let audio = FileManager.default.fileExists(atPath: audioURL.path)
        Section {
            HStack {
                if !audio {
                    Text(Copy.Meetings.noAudio).font(Theme.font(.footnote)).foregroundStyle(Brand.muted.color)
                }
                Spacer()
                Button(Copy.Meetings.stopAudio) { player.stop() }
                    .buttonStyle(.borderless)
                    .disabled(!player.playing)
            }
            if transcript.isEmpty {
                Text(Copy.Meetings.noTranscript).foregroundStyle(Brand.muted.color)
            }
            ForEach(transcript, id: \.startMs) { segment in
                TranscriptLine(segment: segment, audio: audio && !busy, highlighted: jumpMs == segment.startMs,
                               working: working) {
                    player.play(audioURL, fromMs: segment.startMs)
                } retry: {
                    retranscribe()
                }
            }
        }
        .pennelRow()
        Section {
            Button(Copy.Meetings.recognizeSpeakers) {}
                .disabled(true)
        } footer: {
            Text("\(Copy.Meetings.speakersUnavailable) Op iPhone neemt Pennel één microfoonspoor op; iedereen staat onder ‘Anderen’.")
        }
        .pennelRow()
    }

    // MARK: Acties

    private func actions(_ info: MeetingInfo) -> some View {
        Section {
            Picker(Copy.Meetings.template, selection: $template) {
                ForEach(model.templates, id: \.id) { Text($0.name).tag($0.id) }
            }
            .disabled(busy)
            Button(working ? Copy.Meetings.busy : info.hasSummary ? Copy.Meetings.regenerate : Copy.Meetings.summarize) {
                summarize()
            }
            .disabled(working || busy || !canSummarize)
            Menu(Copy.Meetings.share) {
                Button(Copy.Meetings.copyFormatted) { share(formatted: true, transcript: false) }
                Button(Copy.Meetings.copyMarkdown) { share(formatted: false, transcript: false) }
                Button(Copy.Meetings.copyWithTranscript) { share(formatted: true, transcript: true) }
                Button(Copy.Meetings.shareSheet) { presentShare() }
            }
        } footer: {
            Text(!canSummarize ? Copy.Meetings.chooseModel : busy ? Copy.Meetings.waitForRecording : message)
        }
        .pennelRow()
    }

    // MARK: Work

    private func load() async {
        let engine = model.engine
        let id = id
        let loaded = await Task.detached { () -> (MeetingInfo?, String, String?, [Segment]) in
            (engine.meetingInfo(id: id), engine.meetingNotes(id: id), engine.meetingMarkdown(id: id), engine.meetingTranscript(id: id))
        }.value
        info = loaded.0
        savedNotes = loaded.1
        notes = loaded.1
        summaryLines = loaded.2.map { markLines(enhanced: $0, notes: loaded.1) } ?? []
        transcript = loaded.3
        if template.isEmpty { template = model.settings.defaultTemplate }
        if jumpMs != nil, !self.loaded { tab = .transcript }
        self.loaded = true
        loadConflicts()
    }

    private func loadConflicts() {
        conflicts = model.settings.cloudSync ? ConflictCopies.list(in: meetingDirectory) : []
    }

    private func scheduleNotesSave() {
        guard loaded, notes != savedNotes else { return }
        saveNotes?.cancel()
        let text = notes
        saveNotes = Task {
            try? await Task.sleep(for: .milliseconds(600))
            guard !Task.isCancelled else { return }
            writeNotes(text)
        }
    }

    private func flushNotes() {
        guard let pending = saveNotes, loaded else { return }
        pending.cancel()
        saveNotes = nil
        writeNotes(notes)
    }

    private func writeNotes(_ text: String) {
        guard text != savedNotes else { return }
        do {
            try model.engine.setMeetingNotes(id: id, text: text)
            savedNotes = text
            runtime.onUpdated?(id)
        } catch {
            message = ErrorText.failed(error)
        }
    }

    private func summarize() {
        flushNotes()
        working = true
        message = Copy.Meetings.busy
        let engine = model.engine
        let id = id
        let template = template
        Task {
            let result = await Task.detached { Result { try engine.summarizeMeeting(id: id, templateId: template, keys: Keychain.apiKeys()) } }.value
            if case .failure(let error) = result {
                Self.log.error("summary failed: \(ErrorText.code(error), privacy: .public)")
                message = ErrorText.message(error)
            } else {
                message = ""
                runtime.onUpdated?(id)
            }
            working = false
            await load()
        }
    }

    private func retranscribe() {
        working = true
        let engine = model.engine
        let id = id
        Task {
            let result = await Task.detached { Result { try engine.retranscribeFailed(id: id, keys: Keychain.apiKeys()) } }.value
            if case .failure(let error) = result { message = ErrorText.failed(error) } else { CloudSyncRunner.shared.poke() }
            working = false
            await load()
        }
    }

    private func share(formatted: Bool, transcript: Bool) {
        flushNotes()
        Task {
            do {
                MeetingShare.copy(try await MeetingShare.export(model.engine, id: id, transcript: transcript), formatted: formatted)
                message = transcript ? Copy.Meetings.copiedWithTranscript : Copy.Meetings.copied
            } catch {
                message = ErrorText.failed(error)
            }
        }
    }

    private func presentShare() {
        flushNotes()
        Task {
            do {
                MeetingShare.present(try await MeetingShare.export(model.engine, id: id, transcript: false))
            } catch {
                message = ErrorText.failed(error)
            }
        }
    }
}

/// An interrupted meeting never reads as a complete recording.
private struct InterruptionNotice: View {
    let record: MeetingInterruptionLog.Record

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            ForEach(record.gaps, id: \.self) { gap in
                Text(Self.describe(gap))
            }
            if let reason = record.stoppedEarly {
                Text(reason)
            }
        }
        .font(Theme.font(.footnote))
        .foregroundStyle(Brand.accent.color)
    }

    static func describe(_ gap: MeetingGap) -> String {
        let start = Copy.minutesAndSeconds(Double(gap.startMs) / 1000)
        guard let end = gap.endMs else { return "Opname onderbroken vanaf \(start)." }
        return "Opname onderbroken van \(start) tot \(Copy.minutesAndSeconds(Double(end) / 1000))."
    }
}

private struct TranscriptLine: View {
    let segment: Segment
    let audio: Bool
    let highlighted: Bool
    let working: Bool
    let play: () -> Void
    let retry: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: Brand.Spacing.s1) {
            HStack(spacing: Brand.Spacing.s2) {
                Text(Copy.minutesAndSeconds(Double(segment.startMs) / 1000))
                    .monospacedDigit()
                    .foregroundStyle(Brand.muted.color)
                Text(segment.speaker == .me ? "Ik" : "Anderen")
                    .font(Theme.font(.footnote, weight: .semibold))
                    .foregroundStyle(segment.speaker == .me ? Brand.accent.color : Brand.foreground.color)
            }
            .font(Theme.font(.footnote))
            if segment.failed {
                HStack {
                    Text(Copy.Meetings.transcriptionFailed).foregroundStyle(Brand.muted.color)
                    Button(Copy.Meetings.retry, action: retry)
                        .buttonStyle(.borderless)
                        .disabled(working)
                }
            } else {
                Text(segment.text)
            }
        }
        .listRowBackground(highlighted ? Brand.selection.color : Brand.subtle.color)
        .contentShape(Rectangle())
        .contextMenu {
            Button(Copy.Meetings.playFromHere, action: play).disabled(!audio)
        }
        .onTapGesture { if audio { play() } }
        .accessibilityAction(named: Copy.Meetings.playFromHere) { if audio { play() } }
    }
}

/// "Afspelen vanaf dit punt" on `anderen.m4a`, which keeps the transcript's timeline (gaps are silence).
@MainActor
@Observable
final class MeetingPlayer: NSObject, AVAudioPlayerDelegate {
    private(set) var playing = false
    @ObservationIgnored private var player: AVAudioPlayer?
    @ObservationIgnored private let audioSession = SharedAudioSession.shared.client(.playback)
    private static let log = Logger(subsystem: "app.pennel.ios", category: "meeting")

    func play(_ url: URL, fromMs: UInt64) {
        do {
            if player?.url != url {
                player = try AVAudioPlayer(contentsOf: url)
                player?.delegate = self
            }
            try audioSession.activate()
            player?.currentTime = Double(fromMs) / 1000
            player?.play()
            playing = true
        } catch {
            Self.log.error("playback: \(ErrorText.code(error), privacy: .public)")
        }
    }

    func stop() {
        player?.stop()
        playing = false
        audioSession.deactivate()
    }

    /// A finished playback must let go of the session, or its category would block a recording started later.
    nonisolated func audioPlayerDidFinishPlaying(_ player: AVAudioPlayer, successfully flag: Bool) {
        Task { @MainActor in self.stop() }
    }
}
