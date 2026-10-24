import AppKit
@preconcurrency import MusliCore
import MusliKit

/// Watches for calls and applies the user's recording rules.
@MainActor
final class CallCoordinator {
    private let state: AppState
    private let meeting: MeetingController
    private let calendar: CalendarProvider
    private let prompt = RecordPromptPanel()
    private let monitor = MicUsageMonitor(callApps: knownCallApps(), browsers: browserApps())
    private let log = Diagnostics("calls")
    private var detector = CallDetector(ownBundleId: Bundle.main.bundleIdentifier ?? "dev.musli.Musli")
    private var running = false

    init(state: AppState, meeting: MeetingController, calendar: CalendarProvider) {
        self.state = state
        self.meeting = meeting
        self.calendar = calendar
    }

    func refresh() {
        if state.settings.autoDetect, !running {
            running = true
            detector = CallDetector(ownBundleId: Bundle.main.bundleIdentifier ?? "dev.musli.Musli")
            monitor.start { [weak self] users in self?.handle(users) }
        } else if !state.settings.autoDetect, running {
            running = false
            monitor.stop()
            prompt.dismiss()
        }
    }

    private func handle(_ users: Set<MicUser>) {
        for event in detector.update(users: users, at: ProcessInfo.processInfo.systemUptime) {
            switch event {
            case .started(let app): callStarted(app)
            case .ended(let app): callEnded(app)
            }
        }
    }

    private func callStarted(_ app: String) {
        log.notice("call started: \(app)")
        guard !meeting.isRecording else { return }
        let event = state.settings.calendarEnabled ? calendar.currentEvent() : nil
        let action = decideRecording(settings: state.settings, bundleId: app, calendarId: event?.calendarId, eventTitle: event?.title)
        switch action {
        case .never:
            return
        case .always:
            record(app, event)
        case .ask:
            prompt.show(appName: Self.displayName(app), eventTitle: event?.title) { [weak self] answer in
                guard let self else { return }
                switch answer {
                case .yes: self.record(app, event)
                case .no: break
                case .never:
                    self.state.settings.appRules.removeAll { $0.bundleId == app }
                    self.state.settings.appRules.append(AppRule(bundleId: app, action: .never))
                    self.state.save()
                }
            }
        }
    }

    private func record(_ app: String, _ event: CurrentEvent?) {
        meeting.reset()
        meeting.start(title: event?.title, attendees: event?.attendees ?? [], detectedApp: app)
        meeting.onLibraryChanged?()
    }

    private func callEnded(_ app: String) {
        log.notice("call ended: \(app)")
        prompt.dismiss()
        if meeting.isRecording, meeting.detectedApp == app {
            meeting.stop()
            meeting.onLibraryChanged?()
        }
    }

    static func displayName(_ bundleId: String) -> String {
        guard let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundleId) else { return bundleId }
        return FileManager.default.displayName(atPath: url.path).replacingOccurrences(of: ".app", with: "")
    }
}
