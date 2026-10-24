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

    @State private var query = ""
    @State private var meetings: [MeetingInfo] = []
    @State private var selection: String?
    @State private var renaming: MeetingInfo?
    @State private var newTitle = ""
    @State private var deleting: MeetingInfo?
    @State private var asking = false
    @State private var snippets: [String: [String]] = [:]
    @State private var search: Task<Void, Never>?

    var body: some View {
        HStack(spacing: 0) {
            sidebar
                .frame(width: 230)
                .background(Theme.background)
            Group {
                if asking {
                    MeetingChatView(state: state, chat: state.chat(for: nil))
                } else if let selection, meetings.contains(where: { $0.id == selection }) {
                    MeetingDetailView(state: state, meetingId: selection).id(selection)
                } else {
                    Text("Kies een meeting of start een opname.")
                        .foregroundStyle(Theme.muted)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .frame(minWidth: 860, minHeight: 560)
        .background(Theme.paper)
        .onAppear(perform: reload)
        .onChange(of: query) { reload() }
        .onReceive(NotificationCenter.default.publisher(for: .pennelMeetingsChanged)) { _ in reload() }
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

    private var sidebar: some View {
        VStack(alignment: .leading, spacing: 0) {
            TextField("Zoeken", text: $query)
                .textFieldStyle(.roundedBorder)
                .padding(.horizontal, 12)
                .padding(.top, 36)
                .padding(.bottom, 6)
            Button {
                asking = true
            } label: {
                Label("Vraag het je meetings", systemImage: "text.bubble")
                    .font(.figtree(12))
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 6)
                    .background(RoundedRectangle(cornerRadius: 7).fill(asking ? Theme.line : .clear))
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .padding(.horizontal, 8)
            .padding(.bottom, 4)
            if meeting.isRecording {
                Button(action: openRecording) {
                    HStack(spacing: 8) {
                        Circle().fill(Theme.accent).frame(width: 7, height: 7)
                        Text(meeting.title).lineLimit(1)
                        Spacer()
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 8)
                    .background(RoundedRectangle(cornerRadius: 8).fill(Theme.line))
                }
                .buttonStyle(.plain)
                .padding(.horizontal, 8)
            }
            ScrollView {
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(groups, id: \.0) { title, items in
                        Text(title).font(.figtree(11)).foregroundStyle(Theme.muted).padding(.horizontal, 12).padding(.top, 14)
                        ForEach(items, id: \.id) { row($0) }
                    }
                }
                .padding(.horizontal, 6)
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
        let now = Date()
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
        let time = date(info)?.formatted(date: .abbreviated, time: .shortened) ?? ""
        return info.durationMin > 0 ? "\(time) · \(info.durationMin) min" : time
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
            if selection == nil { selection = meetings.first?.id }
        }
    }

    private func rename() {
        guard let info = renaming else { return }
        if let id = try? state.engine.renameMeeting(id: info.id, title: newTitle) { selection = id }
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
