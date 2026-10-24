import EventKit

struct CurrentEvent: Sendable {
    let title: String
    let calendarId: String
    let calendarName: String
    let attendees: [String]
}

final class CalendarProvider {
    private let store = EKEventStore()
    private static let meetingHints = ["teams.microsoft.com", "zoom.us", "meet.google.com", "webex.com", "whereby.com"]

    var hasAccess: Bool {
        EKEventStore.authorizationStatus(for: .event) == .fullAccess
    }

    func requestAccess() async -> Bool {
        (try? await store.requestFullAccessToEvents()) ?? false
    }

    func calendars() -> [(id: String, name: String)] {
        guard hasAccess else { return [] }
        return store.calendars(for: .event).map { ($0.calendarIdentifier, $0.title) }
    }

    /// The event happening now (or starting within 5 minutes), preferring one with a meeting link.
    func currentEvent(now: Date = Date()) -> CurrentEvent? {
        guard hasAccess else { return nil }
        let predicate = store.predicateForEvents(
            withStart: now.addingTimeInterval(-12 * 3600),
            end: now.addingTimeInterval(12 * 3600),
            calendars: nil
        )
        let running = store.events(matching: predicate).filter { event in
            !event.isAllDay && event.startDate.addingTimeInterval(-5 * 60) <= now && now <= event.endDate
        }
        let event = running.first(where: Self.hasMeetingLink) ?? running.first
        return event.map { event in
            CurrentEvent(
                title: event.title ?? "Meeting",
                calendarId: event.calendar.calendarIdentifier,
                calendarName: event.calendar.title,
                attendees: (event.attendees ?? []).filter { !$0.isCurrentUser }.compactMap(\.name)
            )
        }
    }

    private static func hasMeetingLink(_ event: EKEvent) -> Bool {
        let text = [event.url?.absoluteString, event.location, event.notes].compactMap { $0 }.joined(separator: " ")
        return meetingHints.contains { text.localizedCaseInsensitiveContains($0) }
    }
}
