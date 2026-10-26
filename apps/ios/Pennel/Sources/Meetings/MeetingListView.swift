import os
import PennelMobileKit
import SwiftUI

enum MeetingDates {
    static func date(_ iso: String) -> Date? {
        try? Date(iso, strategy: .iso8601)
    }

    static func dayLabel(_ iso: String) -> String {
        guard let date = date(iso) else { return iso }
        return date.formatted(.dateTime.locale(Locale(identifier: "nl-BE")).day().month(.abbreviated).year())
    }

    /// The desktop's `dateLabel`: "24 okt 2026, 09:00".
    static func dateTimeLabel(_ iso: String) -> String {
        [dayLabel(iso), timeLabel(iso)].filter { !$0.isEmpty }.joined(separator: ", ")
    }

    static func timeLabel(_ iso: String) -> String {
        guard let date = date(iso) else { return "" }
        return date.formatted(.dateTime.locale(Locale(identifier: "nl-BE")).hour().minute())
    }

    /// The desktop library's `dayGroup`: **Vandaag**, **Deze week** (the last seven days), **Eerder**; newest first.
    static func groups(_ meetings: [MeetingInfo], now: Date = Date(), calendar: Calendar = .current) -> [(title: String, meetings: [MeetingInfo])] {
        let today = calendar.startOfDay(for: now)
        let week = calendar.date(byAdding: .day, value: -7, to: today) ?? today
        let order = [Copy.Meetings.today, Copy.Meetings.thisWeek, Copy.Meetings.earlier]
        var buckets: [String: [MeetingInfo]] = [:]
        for meeting in meetings {
            let day = date(meeting.startIso) ?? .distantPast
            let title = day >= today ? order[0] : day >= week ? order[1] : order[2]
            buckets[title, default: []].append(meeting)
        }
        return order.compactMap { title in buckets[title].map { (title, $0) } }
    }
}

/// The **Meetings** tab: search, **Nieuwe meeting** with **Start opname**, the live recording and the library by period.
struct MeetingListView: View {
    @Environment(AppModel.self) private var model
    @State private var runtime = MeetingRuntime.shared
    @State private var meetings: [MeetingInfo] = []
    @State private var query = ""
    @State private var path = NavigationPath()
    @State private var startError: String?
    @State private var renaming: MeetingInfo?
    @State private var newTitle = ""
    @State private var newMeetingTitle = ""
    @State private var deleting: MeetingInfo?
    @State private var showsLive = false
    @State private var pruned = false
    @State private var syncStates: [String: SyncItemState] = [:]
    @State private var syncRunner = CloudSyncRunner.shared

    nonisolated private static let log = Logger(subsystem: "app.pennel.ios", category: "meeting")

    var body: some View {
        NavigationStack(path: $path) {
            List {
                Section {
                    if !(runtime.isRecording || runtime.phase == .finishing) {
                        TextField(Copy.Meetings.titlePlaceholder, text: $newMeetingTitle)
                            .accessibilityLabel(Copy.Meetings.meetingTitle)
                            .submitLabel(.go)
                            .onSubmit(startRecording)
                            .pennelRow()
                    }
                    StartRecordingButton(recording: runtime.isRecording || runtime.phase == .finishing) { startRecording() }
                        .listRowBackground(Color.clear)
                        .listRowInsets(EdgeInsets())
                    if let startError {
                        Text(startError).font(Theme.font(.footnote)).foregroundStyle(.red)
                            .listRowBackground(Color.clear)
                    }
                    if runtime.retainedId != nil {
                        RetainedNotice(runtime: runtime)
                    }
                } header: {
                    Text(Copy.Meetings.newMeeting)
                }
                if runtime.isRecording || runtime.phase == .finishing {
                    Section {
                        Button { showsLive = true } label: {
                            HStack {
                                Circle().fill(Brand.accent.color).frame(width: 8, height: 8)
                                Text(runtime.title)
                                Spacer()
                                Text(runtime.phase == .finishing ? Copy.Meetings.finishingRow : Copy.Meetings.recordingRow)
                                    .foregroundStyle(Brand.muted.color)
                            }
                        }
                        .foregroundStyle(Brand.foreground.color)
                        .pennelRow()
                    }
                }
                let shown = meetings.filter { $0.id != runtime.activeMeetingId }
                if shown.isEmpty {
                    Section {
                        VStack(alignment: .leading, spacing: Brand.Spacing.s1) {
                            Text(query.isEmpty ? Copy.Meetings.noMeetings : Copy.Meetings.noResults)
                            if query.isEmpty {
                                Text(Copy.Meetings.emptyHint).foregroundStyle(Brand.muted.color)
                            }
                        }
                        .listRowBackground(Color.clear)
                    }
                }
                ForEach(MeetingDates.groups(shown), id: \.title) { group in
                    Section(group.title) {
                        ForEach(group.meetings, id: \.id) { meeting in
                            NavigationLink(value: MeetingRoute(id: meeting.id, startMs: nil)) {
                                MeetingRow(meeting: meeting, sync: syncStates[meeting.id], syncOn: syncOn)
                            }
                            .pennelRow()
                            .swipeActions {
                                Button(Copy.Meetings.deleteConfirm, role: .destructive) { deleting = meeting }
                            }
                            .contextMenu {
                                Button(Copy.Meetings.rename) {
                                    newTitle = meeting.title
                                    renaming = meeting
                                }
                                Button(Copy.Meetings.delete, role: .destructive) { deleting = meeting }
                            }
                        }
                    }
                }
            }
            .pennelList()
            .navigationTitle(Copy.Tab.meetings)
            .searchable(text: $query, prompt: Copy.Meetings.search)
            .navigationDestination(for: MeetingRoute.self) { route in
                MeetingDetailView(id: route.id, jumpMs: route.startMs)
            }
            .fullScreenCover(isPresented: $showsLive) {
                MeetingLiveView(runtime: runtime) { finishedId in
                    showsLive = false
                    reload()
                    if let finishedId { path.append(MeetingRoute(id: finishedId, startMs: nil)) }
                }
            }
            .alert(Copy.Meetings.renameTitle, isPresented: Binding(get: { renaming != nil }, set: { if !$0 { renaming = nil } })) {
                TextField(Copy.Meetings.meetingTitle, text: $newTitle)
                Button("Annuleren", role: .cancel) {}
                Button("Bewaren") { rename() }
            }
            .alert(
                deleting.map { "\($0.title) verwijderen?" } ?? "",
                isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } })
            ) {
                Button("Annuleren", role: .cancel) {}
                Button(Copy.Meetings.deleteConfirm, role: .destructive) { delete() }
            } message: {
                Text(Copy.Meetings.deleteMessage)
            }
        }
        .task(id: query) { await load() }
        .task { prune() }
        .task { showRequestedLiveMeeting() }
        .onChange(of: AppRouter.shared.liveMeetingRequests) { showRequestedLiveMeeting() }
        .onChange(of: runtime.phase) { reload() }
        .onChange(of: syncRunner.report) { loadSyncStates() }
    }

    private var syncOn: Bool {
        SyncBadges.active(settingOn: model.settings.cloudSync, status: syncRunner.report?.status)
    }

    private func reload() {
        Task { await load() }
    }

    private func loadSyncStates() {
        guard syncOn else {
            syncStates = [:]
            return
        }
        let engine = model.engine
        Task {
            let states = await Task.detached { engine.meetingSyncStates() }.value
            syncStates = Dictionary(states.map { ($0.id, $0.state) }, uniquingKeysWith: { $1 })
        }
    }

    private func load() async {
        let engine = model.engine
        let query = query
        meetings = await Task.detached {
            query.trimmingCharacters(in: .whitespaces).isEmpty ? engine.meetings() : engine.searchMeetings(query: query)
        }.value
        loadSyncStates()
    }

    /// Audio older than the retention setting is removed, as on the desktop; notes and transcripts stay.
    private func prune() {
        guard !pruned else { return }
        pruned = true
        let engine = model.engine
        Task.detached {
            let removed = engine.pruneMeetingAudio()
            if removed > 0 { Self.log.info("pruned \(removed, privacy: .public) recordings") }
        }
    }

    private func showRequestedLiveMeeting() {
        if AppRouter.shared.takeLiveMeetingRequest() { showsLive = true }
    }

    private func startRecording() {
        if runtime.isRecording || runtime.phase == .finishing {
            showsLive = true
            return
        }
        startError = nil
        Task {
            guard await MicCapture.requestPermission() else {
                startError = MeetingRuntime.StartError.microphone.errorDescription
                return
            }
            do {
                try await runtime.start(title: newMeetingTitle)
                newMeetingTitle = ""
                showsLive = true
            } catch {
                Self.log.error("meeting start: \(ErrorText.code(error), privacy: .public)")
                startError = (error as? MeetingRuntime.StartError)?.errorDescription ?? ErrorText.failed(error)
            }
        }
    }

    private func rename() {
        guard let meeting = renaming else { return }
        renaming = nil
        let title = newTitle.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !title.isEmpty, title != meeting.title else { return }
        do {
            let id = try model.engine.renameMeeting(id: meeting.id, title: title)
            runtime.renamed(from: meeting.id, to: id)
            syncRunner.poke()
        } catch {
            model.errorMessage = ErrorText.failed(error)
        }
        reload()
    }

    private func delete() {
        guard let meeting = deleting else { return }
        deleting = nil
        do {
            try model.engine.deleteMeeting(id: meeting.id)
            runtime.deleted(meeting.id)
            syncRunner.poke()
        } catch {
            model.errorMessage = ErrorText.failed(error)
        }
        reload()
    }
}

private struct MeetingRow: View {
    let meeting: MeetingInfo
    let sync: SyncItemState?
    let syncOn: Bool

    var body: some View {
        HStack {
            details
            Spacer()
            SyncBadge(state: sync, syncOn: syncOn)
        }
    }

    private var details: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(meeting.title).font(Theme.font(.body, weight: .medium))
            Text([MeetingDates.dateTimeLabel(meeting.startIso),
                  meeting.durationMin > 0 ? "\(meeting.durationMin) min" : nil]
                .compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " · "))
                .font(Theme.font(.footnote))
                .foregroundStyle(Brand.muted.color)
        }
    }
}

/// Ink button with the Soft wave mark (spec §4.1), labelled as the desktop's start button.
private struct StartRecordingButton: View {
    let recording: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: Brand.Spacing.s3) {
                WaveMark(level: recording ? 0.6 : 0)
                    .frame(width: 28, height: 22)
                Text(recording ? Copy.Meetings.recordingRow : Copy.Meetings.startRecording)
                    .font(Theme.font(.headline, weight: .semibold))
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, Brand.Spacing.s4)
            .foregroundStyle(Brand.pillText.color)
            .background(Brand.pillBackground.color, in: RoundedRectangle(cornerRadius: Brand.Radius.panel))
        }
        .buttonStyle(.plain)
    }
}

struct WaveMark: View {
    let level: Float

    var body: some View {
        GeometryReader { proxy in
            let heights = Brand.Wave.barHeights(level: level, height: proxy.size.height)
            HStack(alignment: .center, spacing: proxy.size.width * Brand.Wave.spacingRatio) {
                ForEach(heights.indices, id: \.self) { index in
                    Capsule()
                        .fill(Brand.wave.color)
                        .frame(width: proxy.size.width * Brand.Wave.barWidthRatio, height: heights[index])
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .accessibilityHidden(true)
    }
}

private struct RetainedNotice: View {
    let runtime: MeetingRuntime
    @State private var error: String?
    @State private var working = false

    var body: some View {
        VStack(alignment: .leading, spacing: Brand.Spacing.s2) {
            Text("Bewaar de resterende gegevens voordat je een nieuwe opname start.")
            if let error { Text(error).foregroundStyle(.red) }
            Button(working ? Copy.Meetings.busy : Copy.Meetings.retrySave) {
                working = true
                Task {
                    do { try await runtime.retrySave(); error = nil } catch { self.error = ErrorText.failed(error) }
                    working = false
                }
            }
            .disabled(working)
        }
        .pennelRow()
    }
}
