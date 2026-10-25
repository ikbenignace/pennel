import AppKit
@preconcurrency import PennelCore
import PennelKit
import SwiftUI

extension Notification.Name {
    static let pennelMeetingsChanged = Notification.Name("dev.pennel.meetingsChanged")
    static let pennelSelectMeeting = Notification.Name("dev.pennel.selectMeeting")
}

struct LibraryView: View {
    let state: AppState
    @Bindable var meeting: MeetingController
    let openRecording: () -> Void

    private var query: String {
        get { state.workspace.meetingQuery }
        nonmutating set { state.workspace.meetingQuery = newValue }
    }
    @State private var meetings: [MeetingInfo] = []
    private var selection: String? {
        get { state.workspace.meetingSelection }
        nonmutating set { state.workspace.meetingSelection = newValue }
    }
    @State private var renaming: MeetingInfo?
    @State private var newTitle = ""
    @State private var deleting: MeetingInfo?
    private var asking: Bool {
        get { state.workspace.askingMeetings }
        nonmutating set { state.workspace.askingMeetings = newValue }
    }
    @State private var snippets: [String: [String]] = [:]
    @State private var search: Task<Void, Never>?

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                if asking || selection != nil {
                    Button {
                        asking = false
                        selection = nil
                    } label: { Label("Alle vergaderingen", systemImage: "chevron.left") }
                    .buttonStyle(.plain).font(.figtree(12)).foregroundStyle(Theme.muted)
                } else {
                    Text("Vergaderingen").font(.figtree(21, weight: .semibold))
                }
                Spacer()
                if !asking && selection == nil { Button("Nieuwe opname", action: openRecording) }
            }
            .frame(minHeight: asking || selection != nil ? 0 : 31)
            .padding(.horizontal, asking || selection != nil ? 32 : 37)
            .padding(.top, asking || selection != nil ? 28 : 32)
            .padding(.bottom, asking || selection != nil ? 18 : 27)
            Group {
                if asking {
                    MeetingChatView(state: state, chat: state.chat(for: nil))
                } else if let selection {
                    MeetingDetailView(state: state, meetingId: selection).id(selection)
                } else {
                    overview
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Theme.paper)
        .onAppear(perform: reload)
        .onChange(of: query) { reload() }
        .onReceive(NotificationCenter.default.publisher(for: .pennelMeetingsChanged)) { _ in reload() }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didBecomeKeyNotification)) { _ in reload() }
        .onReceive(NotificationCenter.default.publisher(for: .pennelSelectMeeting)) { note in
            selection = note.object as? String
            asking = false
            reload()
        }
        .alert("Meeting hernoemen", isPresented: Binding(get: { renaming != nil }, set: { if !$0 { renaming = nil } })) {
            TextField("Titel", text: $newTitle)
            Button("Hernoemen") { rename() }
            Button("Annuleren", role: .cancel) {}
        }
        .confirmationDialog(
            "\(deleting?.title ?? "") naar de prullenmand?",
            isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } })
        ) {
            Button("Naar prullenmand", role: .destructive) { delete() }
        } message: {
            Text("Notities, transcript en audio gaan naar de prullenmand.")
        }
    }

    private var overview: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 15) {
                TextField("Zoeken", text: Binding(get: { query }, set: { query = $0 }))
                    .textFieldStyle(.plain).font(.figtree(12)).padding(.horizontal, 8)
                    .frame(width: 245, height: 31)
                    .background(RoundedRectangle(cornerRadius: 6).fill(Theme.field))
                    .overlay(RoundedRectangle(cornerRadius: 6).stroke(Theme.line))
                Spacer()
                Button("Vraag het je meetings") { asking = true }
                    .buttonStyle(.plain).font(.figtree(12)).foregroundStyle(Theme.muted)
            }
            .padding(.horizontal, 37).padding(.bottom, 20)
            if meetings.isEmpty {
                VStack(spacing: 0) {
                    PennelIcon.notebook.image.resizable().scaledToFit().frame(width: 27, height: 27)
                        .foregroundStyle(Theme.muted).padding(.bottom, 18)
                    Text(query.isEmpty ? "Nog geen vergaderingen." : "Geen meetings gevonden.")
                        .font(.figtree(17, weight: .medium)).padding(.bottom, 18)
                    Text(query.isEmpty ? "Start een opname. Je notities en transcript komen hier." : "Probeer een andere zoekterm.")
                        .font(.figtree(13)).foregroundStyle(Theme.muted)
                        .multilineTextAlignment(.center).frame(maxWidth: 290).lineSpacing(4)
                    if query.isEmpty {
                        Button("Start je eerste opname", action: openRecording)
                            .buttonStyle(PennelPrimaryButtonStyle()).padding(.top, 23)
                    }
                }
                .frame(maxWidth: .infinity).padding(.vertical, 67).padding(.horizontal, 10)
                Spacer(minLength: 0)
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(groups, id: \.0) { title, items in
                            Text(title).font(.figtree(11)).foregroundStyle(Theme.muted).padding(.top, 16)
                            ForEach(items, id: \.id) { row($0) }
                        }
                    }
                    .padding(.horizontal, 32).padding(.bottom, 24)
                }
            }
        }
    }

    private func row(_ info: MeetingInfo) -> some View {
        VStack(alignment: .leading, spacing: 1) {
            Text(info.title).font(.figtree(13)).lineLimit(1)
            Text(Self.subtitle(info)).font(.figtree(11)).foregroundStyle(Theme.muted)
            ForEach(Array((snippets[info.id] ?? []).enumerated()), id: \.offset) { _, snippet in
                Text(Self.highlighted(snippet))
                    .font(.figtree(11))
                    .foregroundStyle(Theme.ink.opacity(0.75))
                    .lineLimit(2)
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 7).fill(selection == info.id && !asking ? Theme.line : .clear))
        .contentShape(Rectangle())
        .onTapGesture {
            selection = info.id
            asking = false
        }
        .contextMenu {
            if info.id == meeting.meetingId && meeting.isRecording {
                Text("Opname loopt")
            } else {
                menuItems(info)
            }
        }
    }

    @ViewBuilder
    private func menuItems(_ info: MeetingInfo) -> some View {
            Button("Hernoemen…") {
                newTitle = info.title
                renaming = info
            }
            Button("Toon in Finder") {
                NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: state.engine.meetingDir(id: info.id))])
            }
            Divider()
            Button("Naar prullenmand…", role: .destructive) { deleting = info }
    }

    private var groups: [(String, [MeetingInfo])] {
        let calendar = Calendar.current
        let now = state.now
        var today: [MeetingInfo] = [], week: [MeetingInfo] = [], older: [MeetingInfo] = []
        for info in meetings {
            let date = Self.date(info) ?? .distantPast
            if calendar.isDateInToday(date) {
                today.append(info)
            } else if let days = calendar.dateComponents([.day], from: date, to: now).day, days < 7 {
                week.append(info)
            } else {
                older.append(info)
            }
        }
        return [("Vandaag", today), ("Deze week", week), ("Eerder", older)].filter { !$0.1.isEmpty }
    }

    static func date(_ info: MeetingInfo) -> Date? {
        ISO8601DateFormatter().date(from: info.startIso)
    }

    static func subtitle(_ info: MeetingInfo) -> String {
        let time = date(info).map(formattedDate) ?? ""
        return info.durationMin > 0 ? "\(time) · \(info.durationMin) min" : time
    }

    static func formattedDate(_ date: Date) -> String {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "nl_BE")
        formatter.dateFormat = "d MMM yyyy, HH:mm"
        return formatter.string(from: date)
    }

    static func highlighted(_ snippet: String) -> AttributedString {
        var result = AttributedString()
        for (index, part) in snippet.components(separatedBy: CharacterSet(charactersIn: "«»")).enumerated() {
            var piece = AttributedString(part)
            if index % 2 == 1 { piece.font = .figtree(11, weight: .semibold) }
            result.append(piece)
        }
        return result
    }

    /// Searches off the main thread; typing quickly only runs the last query.
    private func reload() {
        search?.cancel()
        let (engine, query) = (state.engine, query)
        search = Task {
            if !query.isEmpty { try? await Task.sleep(for: .milliseconds(250)) }
            guard !Task.isCancelled else { return }
            let result = await Task.detached { engine.searchLibrary(query: query) }.value
            guard !Task.isCancelled else { return }
            meetings = result.meetings
            snippets = Dictionary(grouping: result.passages, by: \.meetingId).mapValues { Array($0.prefix(2).map(\.snippet)) }
        }
    }

    private func rename() {
        guard let info = renaming else { return }
        if let id = try? state.engine.renameMeeting(id: info.id, title: newTitle) {
            selection = id
            if let draft = state.workspace.meetingNoteDrafts.removeValue(forKey: info.id) { state.workspace.meetingNoteDrafts[id] = draft }
        }
        renaming = nil
        reload()
    }

    private func delete() {
        guard let info = deleting else { return }
        try? FileManager.default.trashItem(at: URL(fileURLWithPath: state.engine.meetingDir(id: info.id)), resultingItemURL: nil)
        if selection == info.id { selection = nil }
        deleting = nil
        reload()
    }
}
