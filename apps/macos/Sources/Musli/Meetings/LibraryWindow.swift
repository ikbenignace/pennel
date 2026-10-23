import AppKit
@preconcurrency import MusliCore
import MusliKit
import SwiftUI

extension Notification.Name {
    static let musliMeetingsChanged = Notification.Name("dev.musli.meetingsChanged")
    static let musliSelectMeeting = Notification.Name("dev.musli.selectMeeting")
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

    var body: some View {
        HStack(spacing: 0) {
            sidebar
                .frame(width: 230)
                .background(Theme.background)
            Group {
                if let selection, meetings.contains(where: { $0.id == selection }) {
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
        .onReceive(NotificationCenter.default.publisher(for: .musliMeetingsChanged)) { _ in reload() }
        .onReceive(NotificationCenter.default.publisher(for: .musliSelectMeeting)) { note in
            selection = note.object as? String
            reload()
        }
        .alert("Meeting hernoemen", isPresented: Binding(get: { renaming != nil }, set: { if !$0 { renaming = nil } })) {
            TextField("Titel", text: $newTitle)
            Button("Hernoemen") { rename() }
            Button("Annuleren", role: .cancel) {}
        }
        .confirmationDialog(
            "\(deleting?.title ?? "") verwijderen?",
            isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } })
        ) {
            Button("Verwijderen", role: .destructive) { delete() }
        } message: {
            Text("Notities, transcript en audio van deze meeting worden verwijderd.")
        }
    }

    private var sidebar: some View {
        VStack(alignment: .leading, spacing: 0) {
            TextField("Zoeken", text: $query)
                .textFieldStyle(.roundedBorder)
                .padding(.horizontal, 12)
                .padding(.top, 36)
                .padding(.bottom, 8)
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
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 6)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 7).fill(selection == info.id ? Theme.line : .clear))
        .contentShape(Rectangle())
        .onTapGesture { selection = info.id }
        .contextMenu {
            Button("Hernoemen…") {
                newTitle = info.title
                renaming = info
            }
            Button("Toon in Finder") {
                NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: state.engine.meetingDir(id: info.id))])
            }
            Divider()
            Button("Verwijderen…", role: .destructive) { deleting = info }
        }
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

    private func reload() {
        meetings = state.engine.searchMeetings(query: query)
        if selection == nil { selection = meetings.first?.id }
    }

    private func rename() {
        guard let info = renaming else { return }
        if let id = try? state.engine.renameMeeting(id: info.id, title: newTitle) { selection = id }
        renaming = nil
        reload()
    }

    private func delete() {
        guard let info = deleting else { return }
        try? state.engine.deleteMeeting(id: info.id)
        if selection == info.id { selection = nil }
        deleting = nil
        reload()
    }
}
