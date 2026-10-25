import AppKit
@preconcurrency import PennelCore
import PennelKit
import SwiftUI

private var aiInk: Color { Theme.muted }

struct MeetingWindowView: View {
    @Bindable var state: AppState
    @Bindable var meeting: MeetingController

    var body: some View {
        Group {
            switch meeting.phase {
            case .idle:
                VStack(alignment: .leading, spacing: 22) {
                    Text("Je eerste meeting opnemen").font(.figtree(22, weight: .semibold))
                    Text("Je microfoon en de andere deelnemers, in één opname.").font(.figtree(13)).foregroundStyle(Theme.muted)
                    HStack {
                        Text("Microfoon")
                        Spacer()
                        if Permissions.microphone == .authorized { Label("Beschikbaar", systemImage: "checkmark").foregroundStyle(Theme.muted) }
                        else { Button("Geef toegang") { Task { _ = await Permissions.requestMicrophone(); state.workspace.permissionRevision += 1 } } }
                    }.font(.figtree(12))
                    Divider().overlay(Theme.line)
                    HStack {
                        Text("Systeemaudio")
                        Spacer()
                        Text("Wordt bij het starten gecontroleerd").foregroundStyle(Theme.muted)
                    }.font(.figtree(12))
                    Divider().overlay(Theme.line)
                    HStack {
                        Text("Spraakherkenning")
                        Spacer()
                        if state.meetingSetup.configured { Text("Klaar").foregroundStyle(Theme.muted) }
                        else {
                            Button("Kies een model") {
                                state.workspace.destination = .settings
                                state.workspace.settingsTab = .models
                            }
                        }
                    }.font(.figtree(12))
                    Text("Notities, transcript en audio worden in je meetingmap bewaard. Laat deelnemers weten dat je opneemt.")
                        .font(.figtree(11)).foregroundStyle(Theme.muted)
                    if let error = meeting.error {
                        Text(error).foregroundStyle(Theme.accent)
                    }
                    Button("Start opname") { meeting.start() }.disabled(!state.meetingSetup.canRecord)
                    Spacer()
                }
                .padding(36).frame(maxWidth: 740, maxHeight: .infinity, alignment: .topLeading)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            case .recording:
                RecordingView(meeting: meeting)
            case .summarizing:
                VStack(spacing: 10) {
                    ProgressView().controlSize(.small)
                    Text(meeting.willSummarize ? "Bezig met samenvatten…" : "Opname afronden…").foregroundStyle(Theme.muted)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            case .finished:
                if let id = meeting.meetingId {
                    MeetingDetailView(state: state, meetingId: id, notice: meeting.error)
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
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
                .font(.figtree(22, weight: .semibold))
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
                Text("Nog geen geluid van je computer ontvangen. Hoor je de anderen? Geef Pennel toegang bij Systeeminstellingen → Privacy en beveiliging → Systeemaudio-opname.")
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

    private var tab: MeetingDetailTab {
        get { state.workspace.meetingTabs[meetingId] ?? .notes }
        nonmutating set { state.workspace.meetingTabs[meetingId] = newValue }
    }
    @State private var info: MeetingInfo?
    @State private var markdown: String?
    @State private var notes = ""
    @State private var segments: [Segment] = []
    @State private var template = "algemeen"
    @State private var working = false
    @State private var message: String?
    @State private var highlightMs: UInt64?
    @State private var scrollToken = UUID()
    @State private var copied: String?
    @State private var speakers: [SpeakerName] = []
    @State private var attendees: [String] = []
    @State private var renamingVoice: UInt32?
    @State private var speakerName = ""
    @State private var editingAttendees = false
    @State private var attendeesDraft = ""
    @State private var diarizing = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(info?.title ?? meetingId).font(.figtree(22, weight: .semibold))
                    Text(meta).font(.figtree(12)).foregroundStyle(Theme.muted)
                }
                Spacer()
            }
            .padding(.horizontal, 32)
            .padding(.top, 28)
            .padding(.bottom, 14)
            UnderlineTabs(tabs: [(.notes, "Notities"), (.transcript, "Transcript"), (.ask, "Vragen")], selection: Binding(get: { tab }, set: { tab = $0 }))
                .padding(.horizontal, 32).padding(.bottom, 11)
            if let text = message ?? notice {
                Text(text).font(.figtree(12)).foregroundStyle(Theme.accent).padding(.horizontal, 32)
            }
            if markdown == nil && !state.canSummarize {
                HStack(spacing: 10) {
                    Text("Kies een taalmodel om een samenvatting te maken.")
                    Button {
                        state.workspace.destination = .settings
                        state.workspace.settingsTab = .models
                    } label: { Text("Modellen instellen").underline() }
                    .buttonStyle(.plain)
                }
                .font(.figtree(11)).foregroundStyle(Theme.muted).padding(.horizontal, 32).padding(.bottom, 12)
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
                                VStack(alignment: .leading, spacing: 14) {
                                    HStack(spacing: 12) {
                                        if !hasAudio { Text("Geen bewaarde audio beschikbaar.").font(.figtree(11)).foregroundStyle(Theme.muted) }
                                        Spacer(minLength: 0)
                                        Button("Stop audio") { AudioScrubber.shared.stop() }
                                            .buttonStyle(.plain).font(.figtree(11)).foregroundStyle(Theme.muted)
                                            .disabled(!hasAudio)
                                    }
                                    TranscriptList(
                                        segments: segments,
                                        audioDir: audioDir,
                                        onRetry: diarizing ? nil : Optional(retryFailed),
                                        highlightMs: highlightMs,
                                        names: speakers,
                                        suggestions: attendees,
                                        onRename: rename
                                    )
                                }
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
        .onDisappear { state.flushWorkspaceDrafts() }
        .alert(editingAttendees ? "Deelnemers" : "Naam voor deze stem", isPresented: Binding(get: { renamingVoice != nil || editingAttendees }, set: { if !$0 { renamingVoice = nil; editingAttendees = false } })) {
            TextField(editingAttendees ? "Deelnemers" : "Naam", text: editingAttendees ? $attendeesDraft : $speakerName)
            Button("Bewaar") {
                if editingAttendees { saveAttendees() }
                else if let voice = renamingVoice { rename(voice, speakerName) }
            }
            Button("Annuleren", role: .cancel) {}
        } message: {
            Text(editingAttendees ? "Scheid de namen met een komma." : "Laat leeg om terug ‘Anderen’ te tonen.")
        }
    }

    private var footer: some View {
        VStack(alignment: .leading, spacing: 9) {
            ViewThatFits(in: .horizontal) {
                HStack(spacing: 10) { summaryControls; Spacer(minLength: 0); secondaryControls }
                VStack(alignment: .leading, spacing: 10) {
                    HStack(spacing: 10) { summaryControls; Spacer(minLength: 0) }
                    HStack(spacing: 10) { secondaryControls; Spacer(minLength: 0) }
                }
            }
            if tab == .transcript, let reason = diarizationReason {
                Text(reason).font(.figtree(11)).foregroundStyle(Theme.muted)
            }
        }
        .buttonStyle(PennelControlButtonStyle())
        .padding(.horizontal, 32).padding(.vertical, 14)
        .overlay(alignment: .top) { Rectangle().fill(Theme.line).frame(height: 1) }
    }

    @ViewBuilder
    private var summaryControls: some View {
        TemplatePicker(state: state, selection: $template).frame(width: 150)
        Button(working ? "Bezig…" : (markdown == nil ? "Samenvatten" : "Opnieuw genereren"), action: regenerate)
            .disabled(working || !state.canSummarize).fixedSize()
    }

    @ViewBuilder
    private var secondaryControls: some View {
        if let copied { Text(copied).font(.figtree(11)).foregroundStyle(Theme.muted) }
        if tab == .transcript {
            Button(diarizing ? "Bezig…" : "Sprekers herkennen", action: diarize)
                .disabled(diarizing || !canDiarize).help(diarizationReason ?? "Herkent stemmen met het lokale sprekermodel")
                .fixedSize()
        }
        Menu("Delen") {
            Button("Kopieer opgemaakt") { share { MeetingShare.copyFormatted($0); confirm("Gekopieerd") } }
            Button("Kopieer als Markdown") { share { MeetingShare.copyMarkdown($0); confirm("Gekopieerd") } }
            Button("Kopieer met transcript") { share(transcript: true) { MeetingShare.copyFormatted($0); confirm("Gekopieerd met transcript") } }
            Divider()
            Button("Deelnemers…") { attendeesDraft = attendees.joined(separator: ", "); editingAttendees = true }
            Button("Mail…") { share { MeetingShare.mail($0) } }
            Button("Deel…") { share { MeetingShare.share($0) } }
        }.fixedSize()
        Button("Toon in Finder") { NSWorkspace.shared.activateFileViewerSelecting([audioDir]) }.fixedSize()
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
        DisclosureGroup("Je notities") {
            TextEditor(text: Binding(get: { notes }, set: { text in
                notes = text
                state.workspace.meetingNoteDrafts[meetingId] = text
                if !state.flushWorkspaceDrafts() { message = "Notities bewaren mislukt. Je wijzigingen blijven hier staan." }
            }))
            .font(.figtree(14)).scrollContentBackground(.hidden).padding(10).frame(minHeight: 120)
            .background(RoundedRectangle(cornerRadius: 6).fill(Theme.field))
            .overlay(RoundedRectangle(cornerRadius: 6).stroke(Theme.line))
        }
        .font(.figtree(12)).foregroundStyle(Theme.muted).padding(.top, 20)
    }

    private var audioDir: URL { URL(fileURLWithPath: state.engine.meetingDir(id: meetingId)) }
    private var hasAudio: Bool {
        ["ik.m4a", "anderen.m4a"].contains { FileManager.default.fileExists(atPath: audioDir.appendingPathComponent($0).path) }
    }

    private var meta: String {
        guard let info else { return "" }
        let date = ISO8601DateFormatter().date(from: info.startIso)
        let when = date.map(LibraryView.formattedDate) ?? info.startIso
        return [when, info.durationMin > 0 ? "\(info.durationMin) min" : nil, attendees.isEmpty ? nil : attendees.joined(separator: ", ")].compactMap { $0 }.joined(separator: " · ")
    }

    private func rename(_ voice: UInt32, _ name: String?) {
        guard let name else {
            speakerName = speakers.first { $0.voice == voice }?.name ?? ""
            renamingVoice = voice
            return
        }
        do {
            try state.engine.renameSpeaker(id: meetingId, voice: voice, name: name)
        } catch {
            message = "Hernoemen mislukt: \(error)"
        }
        reload()
    }

    private func saveAttendees() {
        let names = attendeesDraft.split(separator: ",").map { $0.trimmingCharacters(in: .whitespacesAndNewlines) }.filter { !$0.isEmpty }
        do { try state.engine.setMeetingAttendees(id: meetingId, attendees: names) }
        catch { message = "Deelnemers bewaren mislukt: \(error)" }
        editingAttendees = false
        reload()
    }

    /// A meeting still being recorded has no duration yet, and its transcript is still being rewritten.
    private var canDiarize: Bool {
        diarizationReason == nil
    }

    private var diarizationReason: String? {
        if (info?.durationMin ?? 0) <= 0 { return "Rond eerst de opname af." }
        if !segments.contains(where: { $0.speaker == .others && !$0.failed }) { return "Geen transcript van andere sprekers beschikbaar." }
        if !FileManager.default.fileExists(atPath: audioDir.appendingPathComponent("anderen.m4a").path) { return "Geen bewaard ‘Anderen’-spoor beschikbaar." }
        if state.engine.modelState(id: state.engine.diarizerModelId()) != .downloaded { return "Download eerst het sprekermodel bij Instellingen → Meetings." }
        return nil
    }

    private func diarize() {
        diarizing = true
        message = nil
        let (engine, id) = (state.engine, meetingId)
        Task {
            let result = await Task.detached { Result { try MeetingController.diarize(engine: engine, meetingId: id) } }.value
            diarizing = false
            switch result {
            case .success(0): message = "Geen afzonderlijke stemmen gevonden."
            case .success: break
            case .failure(let error): message = "Sprekers herkennen mislukt: \(error)"
            }
            reload()
        }
    }

    private func share(transcript: Bool = false, _ action: @escaping (MeetingExport) -> Void) {
        let (engine, id) = (state.engine, meetingId)
        Task {
            let result = await Task.detached { Result { try engine.exportMeeting(id: id, includeTranscript: transcript) } }.value
            switch result {
            case .success(let export): action(export)
            case .failure(let error): message = "Exporteren mislukt: \(error)"
            }
        }
    }

    private func confirm(_ text: String) {
        copied = text
        Task {
            try? await Task.sleep(for: .seconds(2))
            if copied == text { copied = nil }
        }
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
        notes = state.workspace.meetingNoteDrafts[meetingId] ?? engine.meetingNotes(id: meetingId)
        segments = engine.meetingTranscript(id: meetingId)
        speakers = engine.meetingSpeakers(id: meetingId)
        attendees = engine.meetingAttendees(id: meetingId)
        template = state.templateId(state.settings.defaultTemplate)
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
    var names: [SpeakerName] = []
    var suggestions: [String] = []
    var onRename: ((UInt32, String?) -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if segments.isEmpty {
                Text("Nog geen transcript.").foregroundStyle(Theme.muted)
            }
            ForEach(Array(segments.enumerated()), id: \.offset) { _, segment in
                HStack(alignment: .firstTextBaseline, spacing: 10) {
                    Button(Self.stamp(segment.startMs)) { play(segment) }
                        .buttonStyle(.plain).font(.figtree(11)).foregroundStyle(Theme.muted)
                        .frame(width: 52, alignment: .leading).disabled(!canPlay(segment))
                        .accessibilityLabel("Speel audio vanaf \(Self.stamp(segment.startMs))")
                    speakerLabel(segment)
                        .frame(width: 84, alignment: .leading)
                    if segment.failed {
                        Text("Transcriptie mislukt").foregroundStyle(Theme.muted)
                        if let onRetry { Button("Opnieuw proberen", action: onRetry) }
                    } else {
                        Text(segment.text).textSelection(.enabled)
                            .onTapGesture { play(segment) }
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
                .id(segment.startMs)
            }
        }
    }

    @ViewBuilder
    private func speakerLabel(_ segment: Segment) -> some View {
        let text = Text(Self.label(segment, names))
            .font(.figtree(12, weight: .medium))
            .lineLimit(1)
        if segment.speaker == .me {
            text.foregroundStyle(Theme.accent)
        } else if let voice = segment.voice, let onRename {
            Menu {
                ForEach(suggestions, id: \.self) { name in
                    Button(name) { onRename(voice, name) }
                }
                if !suggestions.isEmpty { Divider() }
                Button("Andere naam…") { onRename(voice, nil) }
            } label: {
                text.foregroundStyle(Theme.voice(voice))
            }
            .menuStyle(.borderlessButton)
            .menuIndicator(.hidden)
            .fixedSize()
        } else {
            text.foregroundStyle(segment.voice.map(Theme.voice) ?? Theme.ink)
        }
    }

    static func label(_ segment: Segment, _ names: [SpeakerName]) -> String {
        guard segment.speaker == .others else { return "Ik" }
        guard let voice = segment.voice else { return "Anderen" }
        return names.first { $0.voice == voice }?.name ?? "Anderen \(voice)"
    }

    private func play(_ segment: Segment) {
        guard canPlay(segment), let audioDir else { return }
        let file = segment.speaker == .me ? "ik.m4a" : "anderen.m4a"
        AudioScrubber.shared.play(audioDir.appendingPathComponent(file), fromMs: segment.startMs)
    }

    private func canPlay(_ segment: Segment) -> Bool {
        guard let audioDir else { return false }
        let file = segment.speaker == .me ? "ik.m4a" : "anderen.m4a"
        return FileManager.default.fileExists(atPath: audioDir.appendingPathComponent(file).path)
    }

    static func stamp(_ ms: UInt64) -> String {
        let total = Int(ms / 1000)
        return total >= 3600
            ? String(format: "%d:%02d:%02d", total / 3600, (total % 3600) / 60, total % 60)
            : String(format: "%02d:%02d", total / 60, total % 60)
    }
}
