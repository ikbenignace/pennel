import Foundation

public enum PillState: Equatable, Sendable {
    case hidden
    case listening
    case handsFree
    case working
    case error(String)
}

/// Derives what the pill shows from everything in flight: an active recording wins,
/// then a short-lived notice, then pending dictations.
public struct DictationStatus: Sendable {
    private var recording: PillState?
    private var inFlight = 0
    private var notice: String?

    public init() {}

    public var pill: PillState {
        if let recording { return recording }
        if let notice { return .error(notice) }
        return inFlight > 0 ? .working : .hidden
    }

    public mutating func recordingStarted(handsFree: Bool) {
        recording = handsFree ? .handsFree : .listening
    }

    public mutating func recordingCancelled() {
        recording = nil
    }

    public mutating func recordingSubmitted() {
        recording = nil
        inFlight += 1
    }

    public mutating func dictationFinished(notice: String?) {
        inFlight = max(0, inFlight - 1)
        if let notice { self.notice = notice }
    }

    public mutating func showNotice(_ message: String) {
        notice = message
    }

    public mutating func noticeExpired() {
        notice = nil
    }
}
