import AppKit
@preconcurrency import PennelCore
import PennelKit
import SwiftUI

struct MainWorkspaceView: View {
    @Environment(\.colorScheme) private var colorScheme
    @Bindable var state: AppState
    @Bindable var meeting: MeetingController
    let calendar: CalendarProvider
    var openOnboarding: () -> Void = {}

    var body: some View {
        HStack(spacing: 0) {
            sidebar
                .frame(width: 182)
                .background(Theme.sidebar)
            Rectangle().fill(Theme.line).frame(width: 1)
            VStack(spacing: 0) {
                HStack {
                    Text(state.workspace.destination == .meetings && state.workspace.showingRecording ? "Opname" : state.workspace.destination.title).font(.figtree(12, weight: .medium)).foregroundStyle(Theme.muted)
                    Spacer()
                    Button {
                        state.settings.appearance = colorScheme == .dark ? .light : .dark
                        state.save()
                    } label: { (colorScheme == .dark ? PennelIcon.sun : PennelIcon.moon).image.resizable().scaledToFit().frame(width: 16, height: 16) }
                    .buttonStyle(.plain).foregroundStyle(Theme.muted).accessibilityLabel("Weergave wijzigen")
                }
                .padding(.horizontal, 24).frame(height: 52).background(Theme.chrome)
                .overlay(alignment: .bottom) { Rectangle().fill(Theme.line).frame(height: 1) }
                Group {
                switch state.workspace.destination {
                case .dictation: DictationWorkspaceView(state: state, openOnboarding: openOnboarding)
                case .meetings:
                    if state.workspace.showingRecording {
                        MeetingWindowView(state: state, meeting: meeting)
                    } else {
                        LibraryView(state: state, meeting: meeting, openRecording: startRecording)
                    }
                case .history: HistoryView(state: state)
                case .settings: SettingsView(state: state, calendar: calendar)
                }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .frame(minWidth: 320, minHeight: 0)
        .background(Theme.paper)
        .font(.figtree(13))
        .foregroundStyle(Theme.ink)
        .buttonStyle(PennelControlButtonStyle())
        .onChange(of: state.settings) { state.save() }
        .onChange(of: state.workspace.destination) { old, _ in
            if old == .dictation {
                state.workspace.flushPad()
                if let session = state.workspace.session, session.kind == .pad { state.dictation?.closeSession(session.id) }
            }
            meeting.flushDrafts()
            state.flushWorkspaceDrafts()
        }
        .onReceive(NotificationCenter.default.publisher(for: .pennelSelectMeeting)) { note in
            if let id = note.object as? String { state.workspace.selectMeeting(id) }
        }
        .onReceive(NotificationCenter.default.publisher(for: .pennelMeetingsChanged)) { _ in
            state.workspace.recentMeetings = Array(state.engine.meetings().prefix(3))
        }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didBecomeKeyNotification)) { _ in state.workspace.permissionRevision += 1 }
        .onChange(of: meeting.phase) { _, phase in
            if phase == .finished, let id = meeting.meetingId { state.workspace.recordingFinished(id) }
        }
        .sheet(item: Binding(get: { state.workspace.templateEditing }, set: { state.workspace.templateEditing = $0 })) { target in
            TemplateEditor(target: target) { name, body in
                do {
                    _ = try state.engine.saveTemplate(id: target.id, name: name, body: body)
                    state.workspace.templateEditing = nil
                    state.reloadTemplates()
                    return nil
                } catch { return "Bewaren mislukt: \(error)" }
            } onCancel: { state.workspace.templateEditing = nil }
        }
    }

    private var sidebar: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .lastTextBaseline, spacing: 3) {
                Text("pennel").font(.custom("Instrument Serif", size: 29))
                Circle().fill(Theme.accent).frame(width: 4, height: 4).padding(.bottom, 4)
            }
            .padding(.horizontal, 10)
            .padding(.top, 26)
            .padding(.bottom, 24)
            ForEach([WorkspaceDestination.dictation, .meetings, .history]) { destination in
                navigationButton(destination)
            }
            if meeting.isRecording {
                Button {
                    state.workspace.destination = .meetings
                    state.workspace.showingRecording = true
                } label: {
                    HStack(spacing: 6) {
                        Circle().fill(Theme.accent).frame(width: 6, height: 6)
                        Text(meeting.title).lineLimit(1)
                    }
                    .font(.figtree(11))
                    .foregroundStyle(Theme.accent)
                    .padding(9)
                }
                .buttonStyle(.plain)
                .padding(.top, 14)
            }
            if !state.workspace.recentMeetings.isEmpty {
                Text("Recente meetings").font(.figtree(11)).foregroundStyle(Theme.muted)
                    .padding(.horizontal, 9).padding(.top, 27).padding(.bottom, 6)
                ForEach(state.workspace.recentMeetings, id: \.id) { info in
                    Button { state.workspace.selectMeeting(info.id) } label: {
                        Text(info.title).font(.figtree(11)).lineLimit(2)
                            .frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 9).padding(.vertical, 7)
                    }
                    .buttonStyle(.plain)
                }
            }
            Spacer(minLength: 24)
            HStack(spacing: 7) {
                Text(state.settings.hotkey.label)
                    .font(.figtree(11)).padding(.horizontal, 4).padding(.vertical, 2)
                    .background(RoundedRectangle(cornerRadius: 4).fill(Theme.chrome))
                    .overlay(RoundedRectangle(cornerRadius: 4).stroke(Theme.line))
                Text("Hou in en spreek").font(.figtree(11, weight: .medium))
            }
            .padding(.horizontal, 9)
            Text("Waar je cursor staat.").font(.figtree(11)).foregroundStyle(Theme.muted)
                .padding(.horizontal, 9).padding(.top, 4).padding(.bottom, 20)
            navigationButton(.settings)
        }
        .padding(.horizontal, 11)
        .padding(.bottom, 16)
    }

    private func navigationButton(_ destination: WorkspaceDestination) -> some View {
        Button {
            state.workspace.destination = destination
            if destination == .meetings { state.workspace.showingRecording = false }
        } label: {
            HStack(spacing: 9) {
                navigationIcon(destination).image.resizable().scaledToFit()
                    .frame(width: 16, height: 16)
                    .foregroundStyle(state.workspace.destination == destination ? Theme.accent : Theme.muted)
                Text(destination.title).font(.figtree(12, weight: state.workspace.destination == destination ? .medium : .regular))
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 9).padding(.vertical, 8)
            .background(RoundedRectangle(cornerRadius: 6).fill(state.workspace.destination == destination ? Theme.selection : .clear))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .padding(.bottom, 3)
        .accessibilityAddTraits(state.workspace.destination == destination ? .isSelected : [])
    }

    private func startRecording() {
        state.workspace.showingRecording = true
        if !meeting.isRecording { meeting.reset(); meeting.start() }
    }

    private func navigationIcon(_ destination: WorkspaceDestination) -> PennelIcon {
        switch destination {
        case .dictation: .mic
        case .meetings: .notebook
        case .history: .history
        case .settings: .settings
        }
    }
}

struct DictationWorkspaceView: View {
    @Bindable var state: AppState
    var openOnboarding: () -> Void

    var body: some View {
        if state.workspace.dictationPadOpen {
            DictationPadView(state: state, openOnboarding: openOnboarding)
        } else {
            ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                Text("Dicteren waar je typt.").font(.figtree(21, weight: .semibold)).padding(.bottom, 10)
                Text("Klik in een tekstveld in een andere app. Houd je sneltoets ingedrukt en spreek. Laat los om je tekst daar te plakken.")
                    .font(.figtree(13)).foregroundStyle(Theme.muted).lineSpacing(4).frame(maxWidth: 570, alignment: .leading).padding(.bottom, 28)
                HStack(spacing: 12) {
                    Text(state.settings.hotkey.label).font(.figtree(14, weight: .medium))
                        .padding(.horizontal, 12).padding(.vertical, 8)
                        .background(RoundedRectangle(cornerRadius: 6).fill(Theme.chrome))
                        .overlay(RoundedRectangle(cornerRadius: 6).stroke(Theme.line))
                    Text("Houd in om te spreken. Laat los om te plakken.").font(.figtree(12)).foregroundStyle(Theme.muted)
                }.padding(.bottom, 24)
                if !state.dictationSetup.canRecord {
                    Text(state.dictationSetup.reason ?? "Stel eerst spraakherkenning in.").font(.figtree(12)).foregroundStyle(Theme.muted).padding(.bottom, 12)
                    Button("Maak het klaar", action: openOnboarding).padding(.bottom, 16)
                } else if !state.dictationSetup.canUseShortcut {
                    Text("Geef Pennel toegang tot Toegankelijkheid om de sneltoets in andere apps te gebruiken.").font(.figtree(12)).foregroundStyle(Theme.muted).padding(.bottom, 12)
                    Button("Controleer toegang") { state.onResumeOnboarding?() }.padding(.bottom, 16)
                }
                Button("Dicteren instellen") { state.workspace.settingsTab = .dictation; state.workspace.destination = .settings }
                Rectangle().fill(Theme.line).frame(height: 1).padding(.vertical, 32)
                Button("Open dicteerpad") { state.workspace.dictationPadOpen = true }.buttonStyle(.plain).font(.figtree(13, weight: .medium))
                Text("Alleen als je tekst eerst in Pennel wilt bewaren of bewerken.").font(.figtree(12)).foregroundStyle(Theme.muted).padding(.top, 7)
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 37).padding(.top, 32).padding(.bottom, 24)
            .frame(maxWidth: 800, maxHeight: .infinity).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
            }
        }
    }
}

private struct DictationPadView: View {
    @Bindable var state: AppState
    var openOnboarding: () -> Void
    private var session: OwnedDictationSession? { state.workspace.session?.kind == .pad ? state.workspace.session : nil }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Button("Terug naar overal dicteren") {
                if state.workspace.flushPad() { state.workspace.dictationPadOpen = false }
            }.buttonStyle(.plain).font(.figtree(12)).foregroundStyle(Theme.muted)
                .disabled(session?.busy == true).padding(.bottom, 19)
            if !state.dictationSetup.canRecord {
                HStack(spacing: 12) {
                    PennelIcon.warning.image.resizable().scaledToFit().frame(width: 16, height: 16).foregroundStyle(Theme.muted)
                    Text(state.dictationSetup.reason ?? "Dicteren is nog niet klaar.").font(.figtree(12)).foregroundStyle(Theme.muted)
                    Spacer()
                    Button("Maak het klaar", action: openOnboarding)
                }
                .padding(.bottom, 18).overlay(alignment: .bottom) { Rectangle().fill(Theme.line).frame(height: 1) }.padding(.bottom, 25)
            }
            HStack(alignment: .center, spacing: 16) {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Dicteerpad").font(.figtree(21, weight: .semibold))
                    Text("Spreek, lees na en werk je tekst bij.").font(.figtree(12)).foregroundStyle(Theme.muted)
                }
                Spacer()
                Button(action: toggle) {
                    HStack(spacing: 7) {
                        (session?.phase == .listening ? PennelIcon.square : PennelIcon.mic).image.resizable().scaledToFit().frame(width: 16, height: 16)
                        Text(session?.phase == .listening ? "Stop dicteren" : session?.phase == .working ? "Tekst verwerken…" : "Start dicteren")
                    }
                }
                .buttonStyle(PennelPrimaryButtonStyle())
                .disabled(state.dictation == nil || session?.phase == .working || !state.dictationSetup.canRecord)
                .keyboardShortcut("r", modifiers: [.command, .option])
            }
            .padding(.bottom, 27)
            HStack(spacing: 10) {
                Text(state.settings.hotkey.label).font(.figtree(11)).foregroundStyle(Theme.muted)
                    .padding(.horizontal, 4).padding(.vertical, 2)
                    .background(RoundedRectangle(cornerRadius: 4).fill(Theme.chrome))
                    .overlay(RoundedRectangle(cornerRadius: 4).stroke(Theme.line))
                Text(state.dictationSetup.canUseShortcut ? "Hou in om te spreken. Twee keer tikken voor handsfree." : "Je kunt de knop gebruiken om hier te dicteren.").font(.figtree(12)).foregroundStyle(Theme.muted)
            }.padding(.bottom, 19)
            if !state.dictationSetup.canUseShortcut {
                HStack(spacing: 10) {
                    PennelIcon.warning.image.resizable().scaledToFit().frame(width: 16, height: 16).foregroundStyle(Theme.muted)
                    Text("Geef Pennel toegang tot Toegankelijkheid om \(state.settings.hotkey.label) in andere apps te gebruiken.")
                        .font(.figtree(12)).foregroundStyle(Theme.muted)
                    Spacer(minLength: 0)
                    Button("Controleer toegang") { state.onResumeOnboarding?() }
                        .buttonStyle(.plain).font(.figtree(12)).foregroundStyle(Theme.muted)
                }
                .padding(.bottom, 18).overlay(alignment: .bottom) { Rectangle().fill(Theme.line).frame(height: 1) }.padding(.bottom, 25)
            }
            TextEditor(text: Binding(get: { state.workspace.padText }, set: { state.workspace.editPad($0) }))
                .font(.figtree(15)).scrollContentBackground(.hidden).padding(13)
                .background(RoundedRectangle(cornerRadius: 6).fill(Theme.field))
                .overlay(RoundedRectangle(cornerRadius: 6).stroke(Theme.line))
                .frame(minHeight: 220, maxHeight: .infinity)
                .disabled(session?.busy == true).accessibilityLabel("Dicteerpad")
            HStack(spacing: 10) {
                Button {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(state.workspace.padText, forType: .string)
                } label: {
                    HStack(spacing: 6) { PennelIcon.copy.image.resizable().scaledToFit().frame(width: 16, height: 16); Text("Kopiëren") }
                }
                .disabled(state.workspace.padText.isEmpty || session?.busy == true)
                if session?.busy == true {
                    Button("Annuleer") { if let session { state.dictation?.cancelSession(session.id) } }
                        .buttonStyle(.plain).font(.figtree(12)).foregroundStyle(Theme.muted)
                }
                Spacer()
                Button {
                    state.workspace.destination = .settings
                    state.workspace.settingsTab = .dictation
                } label: {
                    HStack(spacing: 6) { PennelIcon.settings.image.resizable().scaledToFit().frame(width: 16, height: 16); Text("Dicteren instellen") }
                }
                .buttonStyle(.plain).font(.figtree(12)).foregroundStyle(Theme.muted)
            }.padding(.top, 15)
            if session?.phase == .listening {
                Text("Luistert…").font(.figtree(11)).foregroundStyle(Theme.muted).padding(.top, 15)
            } else if session?.phase == .working {
                Text("Je woorden worden uitgeschreven…").font(.figtree(11)).foregroundStyle(Theme.muted).padding(.top, 15)
            }
            if let error = session?.error ?? state.workspace.padError {
                Text(error).font(.figtree(12)).foregroundStyle(Theme.accent).padding(.top, 15)
            }
            Text(targetDescription).font(.figtree(11)).foregroundStyle(Theme.muted).padding(.top, 15)
        }
        .padding(.horizontal, 37).padding(.top, 32).padding(.bottom, 24)
        .frame(maxWidth: 800, maxHeight: .infinity)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .onAppear { _ = state.dictation?.openSession(kind: .pad) }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didBecomeKeyNotification)) { note in
            if (note.object as? NSWindow)?.identifier?.rawValue == "main", session?.busy != true { _ = state.dictation?.openSession(kind: .pad) }
        }
        .onDisappear {
            state.workspace.flushPad()
            if let session { state.dictation?.closeSession(session.id) }
        }
    }

    private var targetDescription: String {
        guard state.settings.dictationPadAutoPaste else { return "Je tekst blijft in Pennel. Kopieer wanneer je wilt." }
        guard state.dictationSetup.canPasteExternally else { return "Geef Pennel toegang tot Toegankelijkheid om extern te plakken." }
        return session?.targetName.map { "Plakt ook in \($0)." } ?? "Geen doelapp beschikbaar. Je tekst blijft hier."
    }

    private func toggle() {
        guard let manager = state.dictation else { return }
        guard state.workspace.flushPad() else { return }
        if let session { manager.toggleSession(session.id) }
        else if let session = manager.openSession(kind: .pad) { manager.toggleSession(session.id) }
    }
}
