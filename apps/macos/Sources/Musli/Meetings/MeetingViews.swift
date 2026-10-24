import AppKit
@preconcurrency import MusliCore
import MusliKit
import SwiftUI

private let aiInk = Color(hex: 0x8C857B)

struct MeetingWindowView: View {
    @Bindable var state: AppState
    @Bindable var meeting: MeetingController

    var body: some View {
        Group {
            switch meeting.phase {
            case .idle:
                VStack(spacing: 14) {
                    if let error = meeting.error {
                        Text(error).foregroundStyle(Theme.accent)
                    }
                    Button("Start opname") { meeting.start() }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            case .recording:
                RecordingView(meeting: meeting)
            case .summarizing:
                VStack(spacing: 10) {
                    ProgressView().controlSize(.small)
                    Text("Bezig met samenvatten…").foregroundStyle(Theme.muted)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            case .finished:
                if let id = meeting.meetingId {
                    MeetingDetailView(state: state, meetingId: id, notice: meeting.error)
                }
            }
        }
        .frame(minWidth: 620, minHeight: 520)
        .background(Theme.paper)
    }
}

private struct RecordingView: View {
    @Bindable var meeting: MeetingController
    @State private var showTranscript = false

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            TextField("Titel", text: $meeting.title)
                .textFieldStyle(.plain)
                .font(.custom("Instrument Serif", size: 30))
            HStack(spacing: 8) {
                Circle().fill(Theme.accent).frame(width: 8, height: 8)
                Text("Opname loopt · \(Self.clock(meeting.elapsed))").foregroundStyle(Theme.muted)
                Spacer()
                Button("Stop opname") { meeting.stop() }.keyboardShortcut(.defaultAction)
            }
            .font(.figtree(12))
            if meeting.consentCopied {
                Text("Toestemmingszin gekopieerd — plak hem in de chat van je call.")
                    .font(.figtree(12))
                    .foregroundStyle(Theme.muted)
            }
            if meeting.systemAudioMissing {
                Text("Nog geen geluid van je computer ontvangen. Hoor je de anderen? Geef Musli toegang bij Systeeminstellingen → Privacy en beveiliging → Systeemaudio-opname.")
                    .font(.figtree(12))
                    .foregroundStyle(Theme.accent)
            }
            ZStack(alignment: .topLeading) {
                if meeting.notes.isEmpty {
                    Text("Typ steekwoorden… of laat het leeg voor een samenvatting.")
                        .foregroundStyle(Theme.muted)
                        .padding(.top, 8)
                        .padding(.leading, 5)
                }
                TextEditor(text: $meeting.notes)
                    .font(.figtree(14))
                    .scrollContentBackground(.hidden)
            }
            .frame(maxHeight: .infinity)
            DisclosureGroup("Live transcript", isExpanded: $showTranscript) {
                ScrollView {
                    TranscriptList(segments: meeting.liveTranscript, audioDir: nil, onRetry: nil)
                }
                .frame(height: 160)
            }
            .font(.figtree(12))
            .foregroundStyle(Theme.muted)
        }
        .padding(28)
    }

    static func clock(_ seconds: TimeInterval) -> String {
        let total = Int(seconds)
        return total >= 3600
            ? String(format: "%d:%02d:%02d", total / 3600, (total % 3600) / 60, total % 60)
            : String(format: "%02d:%02d", total / 60, total % 60)
    }
}

struct MeetingDetailView: View {
    let state: AppState
    let meetingId: String
    var notice: String?

    private enum Tab: Hashable { case notes, transcript, ask }
    @State private var tab: Tab = .notes
    @State private var info: MeetingInfo?
    @State private var markdown: String?
    @State private var notes = ""
    @State private var segments: [Segment] = []
    @State private var template = "algemeen"
    @State private var working = false
    @State private var message: String?
    @State private var highlightMs: UInt64?
    @State private var scrollToken = UUID()

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(info?.title ?? meetingId).font(.custom("Instrument Serif", size: 30))
                    Text(meta).font(.figtree(12)).foregroundStyle(Theme.muted)
                }
                Spacer()
                SegmentTabs(tabs: [(.notes, "Notities"), (.transcript, "Transcript"), (.ask, "Vragen")], selection: $tab)
            }
            .padding(.horizontal, 32)
            .padding(.top, 28)
            .padding(.bottom, 14)
            if let text = message ?? notice {
                Text(text).font(.figtree(12)).foregroundStyle(Theme.accent).padding(.horizontal, 32)
            }
            if tab == .ask {
                MeetingChatView(state: state, chat: state.chat(for: meetingId))
            } else {
                ScrollViewReader { proxy in
                    ScrollView {
                        Group {
                            switch tab {
                            case .notes, .ask: notesView
                            case .transcript:
                                TranscriptList(
                                    segments: segments,
                                    audioDir: audioDir,
                                    onRetry: retryFailed,
                                    highlightMs: highlightMs
                                )
                            }
                        }
                        .padding(.horizontal, 32)
                        .padding(.vertical, 12)
                        .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .onChange(of: scrollToken, initial: true) {
                        guard let ms = highlightMs, tab == .transcript else { return }
                        DispatchQueue.main.async { withAnimation { proxy.scrollTo(ms, anchor: .center) } }
                    }
                }
                footer
            }
        }
        .background(Theme.paper)
        .onAppear(perform: reload)
        .onChange(of: meetingId) { reload() }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didBecomeKeyNotification)) { _ in reload() }
        .onChange(of: state.transcriptJump, initial: true) { consumeJump() }
    }

    private var footer: some View {
            HStack(spacing: 10) {
                Picker("", selection: $template) {
                    ForEach(meetingTemplates(), id: \.id) { Text($0.name).tag($0.id) }
                }
                .labelsHidden()
                .frame(width: 150)
                Button(working ? "Bezig…" : (markdown == nil ? "Samenvatten" : "Opnieuw genereren"), action: regenerate)
                    .disabled(working)
                Spacer()
                Button("Toon in Finder") {
                    NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: state.engine.meetingDir(id: meetingId))])
                }
            }
            .padding(.horizontal, 32)
            .padding(.vertical, 14)
    }

    @ViewBuilder
    private var notesView: some View {
        if let markdown {
            VStack(alignment: .leading, spacing: 5) {
                ForEach(Array(markLines(enhanced: markdown, notes: notes).enumerated()), id: \.offset) { _, line in
                    MarkdownLine(text: line.text, mine: line.mine)
                }
            }
        } else if !notes.isEmpty {
            Text(notes).font(.figtree(14))
        } else {
            Text("Nog geen samenvatting.").foregroundStyle(Theme.muted)
        }
    }

    private var audioDir: URL { URL(fileURLWithPath: state.engine.meetingDir(id: meetingId)) }

    private var meta: String {
        guard let info else { return "" }
        let date = ISO8601DateFormatter().date(from: info.startIso)
        let when = date?.formatted(date: .complete, time: .shortened) ?? info.startIso
        return info.durationMin > 0 ? "\(when) · \(info.durationMin) min" : when
    }

    private func consumeJump() {
        guard let jump = state.transcriptJump, jump.meetingId == meetingId else { return }
        state.transcriptJump = nil
        if segments.isEmpty { segments = state.engine.meetingTranscript(id: meetingId) }
        tab = .transcript
        highlightMs = segments.last(where: { $0.startMs <= jump.ms })?.startMs ?? jump.ms
        scrollToken = UUID()
    }

    private func reload() {
        let engine = state.engine
        info = engine.meetingInfo(id: meetingId)
        markdown = engine.meetingMarkdown(id: meetingId)
        notes = engine.meetingNotes(id: meetingId)
        segments = engine.meetingTranscript(id: meetingId)
        template = state.settings.defaultTemplate
    }

    private func regenerate() {
        working = true
        message = nil
        let engine = state.engine
        let keys = state.keys()
        let id = meetingId
        let template = template
        Task {
            let result = await Task.detached { Result { try engine.summarizeMeeting(id: id, templateId: template, keys: keys) } }.value
            working = false
            if case .failure(let error) = result { message = "Samenvatten mislukt: \(error)" }
            reload()
        }
    }

    private func retryFailed() {
        let engine = state.engine
        let keys = state.keys()
        let id = meetingId
        Task {
            let result = await Task.detached { Result { try engine.retranscribeFailed(id: id, keys: keys) } }.value
            if case .failure(let error) = result { message = "Opnieuw proberen mislukt: \(error)" }
            reload()
        }
    }
}

private struct MarkdownLine: View {
    let text: String
    let mine: Bool

    var body: some View {
        let trimmed = text.trimmingCharacters(in: .whitespaces)
        if trimmed.hasPrefix("## ") {
            Text(trimmed.dropFirst(3)).font(.figtree(13, weight: .semibold)).padding(.top, 12)
        } else if trimmed.isEmpty {
            Color.clear.frame(height: 2)
        } else {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                if let bullet = Self.bullet(trimmed) { Text(bullet).foregroundStyle(Theme.muted) }
                Text(LocalizedStringKey(Self.content(trimmed)))
                    .foregroundStyle(mine ? Theme.ink : aiInk)
                    .textSelection(.enabled)
            }
            .font(.figtree(14))
        }
    }

    static func bullet(_ line: String) -> String? {
        if line.hasPrefix("- [ ] ") { return "☐" }
        if line.hasPrefix("- [x] ") { return "☑︎" }
        if line.hasPrefix("- ") || line.hasPrefix("* ") { return "•" }
        return nil
    }

    static func content(_ line: String) -> String {
        for prefix in ["- [ ] ", "- [x] ", "- ", "* "] where line.hasPrefix(prefix) {
            return String(line.dropFirst(prefix.count))
        }
        return line
    }
}

struct TranscriptList: View {
    let segments: [Segment]
    let audioDir: URL?
    let onRetry: (() -> Void)?
    var highlightMs: UInt64?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if segments.isEmpty {
                Text("Nog geen transcript.").foregroundStyle(Theme.muted)
            }
            ForEach(Array(segments.enumerated()), id: \.offset) { _, segment in
                HStack(alignment: .firstTextBaseline, spacing: 10) {
                    Text(Self.stamp(segment.startMs)).font(.figtree(11)).foregroundStyle(Theme.muted).frame(width: 52, alignment: .leading)
                    Text(segment.speaker == .me ? "Ik" : "Anderen")
                        .font(.figtree(12, weight: .medium))
                        .foregroundStyle(segment.speaker == .me ? Theme.accent : Theme.ink)
                        .frame(width: 60, alignment: .leading)
                    if segment.failed {
                        Text("Transcriptie mislukt").foregroundStyle(Theme.muted)
                        if let onRetry { Button("Opnieuw proberen", action: onRetry) }
                    } else {
                        Text(segment.text).textSelection(.enabled)
                    }
                    Spacer()
                }
                .font(.figtree(13))
                .padding(.vertical, 2)
                .background(
                    RoundedRectangle(cornerRadius: 6)
                        .fill(segment.startMs == highlightMs ? Theme.wave.opacity(0.35) : .clear)
                        .padding(.horizontal, -6)
                )
                .contentShape(Rectangle())
                .onTapGesture { play(segment) }
                .id(segment.startMs)
            }
        }
    }

    private func play(_ segment: Segment) {
        guard let audioDir else { return }
        let file = segment.speaker == .me ? "ik.m4a" : "anderen.m4a"
        AudioScrubber.shared.play(audioDir.appendingPathComponent(file), fromMs: segment.startMs)
    }

    static func stamp(_ ms: UInt64) -> String {
        let total = Int(ms / 1000)
        return total >= 3600
            ? String(format: "%d:%02d:%02d", total / 3600, (total % 3600) / 60, total % 60)
            : String(format: "%02d:%02d", total / 60, total % 60)
    }
}
