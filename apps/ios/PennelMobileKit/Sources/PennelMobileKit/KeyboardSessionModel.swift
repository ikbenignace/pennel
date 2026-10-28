import Foundation
import Observation

/// What the dictation pad shows. Spec 4.5 lists one row per case.
public enum KeyboardPad: Equatable {
    case needsFullAccess, sessionOff, ready, listening, processing
    case failed(String)
}

/// UIKit-free state shared by the extension and Swift Package tests. The handoff carries commands and
/// explicit dictation results; unrelated typed text never enters this model.
@MainActor
@Observable
public final class KeyboardSessionModel {
    /// Commands are fire and forget; if the app has not reflected a start by then, it never acted on it.
    public static let acknowledgementGrace: TimeInterval = 1.5
    public static let resultGrace: TimeInterval = 3

    public private(set) var pad: KeyboardPad
    public private(set) var lastResult: HandoffResult?
    public private(set) var level: Float = 0

    private enum Gesture { case none, hold, handsFree }

    private let hasFullAccess: Bool
    private let client: HandoffClient
    private let now: () -> Date
    private let context: () -> KeyboardInsertionContext?
    private let onResult: (String) -> Void
    public let copy: KeyboardCopy

    private var gesture = Gesture.none
    private var startedAt: Date?
    private var stoppedAt: Date?
    private var failure: String?
    private var delivery = KeyboardDeliveryGuard()

    public init(
        hasFullAccess: Bool, client: HandoffClient, now: @escaping () -> Date = Date.init,
        context: @escaping () -> KeyboardInsertionContext?,
        copy: KeyboardCopy = KeyboardCopy(language: .english), onResult: @escaping (String) -> Void
    ) {
        self.hasFullAccess = hasFullAccess
        self.client = client
        self.now = now
        self.context = context
        self.copy = copy
        self.onResult = onResult
        pad = hasFullAccess ? .sessionOff : .needsFullAccess
    }

    public func refresh() {
        guard hasFullAccess else {
            pad = .needsFullAccess
            return
        }
        let current = now()
        if let result = client.pendingResult(now: current) {
            if result.text == nil || delivery.permits(result.id, in: context()) {
                if client.markInserted(result) { finish(result) }
            } else {
                client.deferPendingResult(result)
                reset()
            }
        }
        let session = client.session(now: current)
        level = session == .listening ? client.level() : 0
        lastResult = isIdle ? client.lastUninserted(now: current) : nil

        if let failure {
            pad = .failed(failure)
            return
        }
        switch session {
        case .off:
            reset()
            pad = .sessionOff
        case .listening:
            // The app may still read listening for a moment after our stop; it must not look tappable again.
            if let stoppedAt, current.timeIntervalSince(stoppedAt) < Self.resultGrace {
                pad = .processing
            } else {
                stoppedAt = nil
                pad = .listening
            }
        case .processing:
            pad = .processing
        case .ready:
            if let startedAt, gesture != .none, current.timeIntervalSince(startedAt) < Self.acknowledgementGrace {
                pad = .listening
            } else if let stoppedAt, current.timeIntervalSince(stoppedAt) < Self.resultGrace {
                pad = .processing
            } else {
                reset()
                pad = .ready
            }
        }
    }

    public func tapPill() {
        guard hasFullAccess else { return }
        switch pad {
        case .sessionOff:
            break
        case .ready:
            begin(.handsFree)
        case .listening:
            if gesture != .hold { end() }
        case .failed:
            retry()
        case .needsFullAccess, .processing:
            break
        }
    }

    public func holdBegan() {
        guard hasFullAccess else { return }
        switch pad {
        case .ready: begin(.hold)
        case .sessionOff: break
        default: break
        }
    }

    public func holdEnded() {
        if pad == .listening, gesture == .hold { end() }
    }

    public func cancel() {
        guard hasFullAccess, pad == .listening || pad == .processing else { return }
        _ = send(.cancel)
        reset()
        refresh()
    }

    public func retry() {
        failure = nil
        reset()
        refresh()
    }

    public func insertLast() {
        guard let result = lastResult, context() != nil else { return }
        lastResult = nil
        if client.markInserted(result), let text = result.text { onResult(text) }
    }

    public func contextDidChange() {
        delivery.invalidate()
        if pad == .listening, gesture != .none { end() }
    }

    private var isIdle: Bool { gesture == .none && stoppedAt == nil }

    private func begin(_ new: Gesture) {
        guard let origin = context() else { return }
        delivery.begin(in: origin)
        guard send(.start) != nil else { return }
        gesture = new
        startedAt = now()
        stoppedAt = nil
        pad = .listening
    }

    private func end() {
        // A second stop would replace the pending id and orphan the first result.
        guard stoppedAt == nil, let id = send(.stop) else { return }
        delivery.awaitResult(id)
        gesture = .none
        stoppedAt = now()
        pad = .processing
    }

    @discardableResult
    private func send(_ action: HandoffCommand.Action) -> UUID? {
        do {
            return try client.send(action, now: now())
        } catch {
            failure = copy.text(.fullAccess)
            pad = .failed(copy.text(.fullAccess))
            return nil
        }
    }

    private func finish(_ result: HandoffResult) {
        reset()
        if let text = result.text {
            onResult(text)
        } else {
            failure = result.error ?? copy.text(.retry)
        }
    }

    private func reset() {
        gesture = .none
        startedAt = nil
        stoppedAt = nil
        delivery.reset()
    }
}
