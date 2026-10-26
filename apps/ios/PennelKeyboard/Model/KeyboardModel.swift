import Foundation
import Observation
import PennelMobileKit

/// What the dictation pad shows. Spec 4.5 lists one row per case.
enum KeyboardPad: Equatable {
    case needsFullAccess, sessionOff, openPennelFallback, ready, listening, processing
    case failed(String)
}

/// No UIKit: the extension cannot host tests, so this file is also compiled into `PennelKeyboardTests`.
/// It reads `session.json` / `result.json` and writes `command.json` through `HandoffClient`; it never stores
/// or logs text, only hands a finished result to `onResult` once.
@MainActor
@Observable
final class KeyboardModel {
    /// Commands are fire and forget; if the app has not reflected a start by then, it never acted on it.
    static let acknowledgementGrace: TimeInterval = 1.5
    static let resultGrace: TimeInterval = 3

    private(set) var pad: KeyboardPad
    private(set) var lastResult: HandoffResult?
    private(set) var level: Float = 0

    private enum Gesture { case none, hold, handsFree }

    private let hasFullAccess: Bool
    private let client: HandoffClient
    private let now: () -> Date
    private let openApp: () -> Bool
    private let onResult: (String) -> Void

    private var gesture = Gesture.none
    private var startedAt: Date?
    private var stoppedAt: Date?
    private var failure: String?
    private var fallbackShown = false

    init(
        hasFullAccess: Bool, client: HandoffClient, now: @escaping () -> Date = Date.init,
        openApp: @escaping () -> Bool, onResult: @escaping (String) -> Void
    ) {
        self.hasFullAccess = hasFullAccess
        self.client = client
        self.now = now
        self.openApp = openApp
        self.onResult = onResult
        pad = hasFullAccess ? .sessionOff : .needsFullAccess
    }

    func refresh() {
        guard hasFullAccess else {
            pad = .needsFullAccess
            return
        }
        let current = now()
        if let result = client.takeResult() { finish(result) }
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
            pad = fallbackShown ? .openPennelFallback : .sessionOff
        case .listening:
            pad = .listening
        case .processing:
            stoppedAt = stoppedAt ?? current
            pad = .processing
        case .ready:
            fallbackShown = false
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

    func tapPill() {
        guard hasFullAccess else { return }
        switch pad {
        case .sessionOff, .openPennelFallback:
            fallbackShown = !openApp()
            refresh()
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

    func holdBegan() {
        guard hasFullAccess else { return }
        switch pad {
        case .ready: begin(.hold)
        case .sessionOff, .openPennelFallback: tapPill()
        default: break
        }
    }

    func holdEnded() {
        if pad == .listening, gesture == .hold { end() }
    }

    func cancel() {
        guard hasFullAccess, pad == .listening else { return }
        send(.cancel)
        reset()
        refresh()
    }

    func retry() {
        failure = nil
        reset()
        refresh()
    }

    func insertLast() {
        guard let result = lastResult else { return }
        lastResult = nil
        client.markInserted(result)
        if let text = result.text { onResult(text) }
    }

    private var isIdle: Bool { gesture == .none && stoppedAt == nil }

    private func begin(_ new: Gesture) {
        guard send(.start) else { return }
        gesture = new
        startedAt = now()
        stoppedAt = nil
        pad = .listening
    }

    private func end() {
        guard send(.stop) else { return }
        gesture = .none
        stoppedAt = now()
        pad = .processing
    }

    @discardableResult
    private func send(_ action: HandoffCommand.Action) -> Bool {
        do {
            try client.send(action, now: now())
            return true
        } catch {
            failure = Copy.Keyboard.fullAccessNeeded
            pad = .failed(Copy.Keyboard.fullAccessNeeded)
            return false
        }
    }

    private func finish(_ result: HandoffResult) {
        reset()
        if let text = result.text {
            onResult(text)
        } else {
            failure = result.error ?? Copy.Keyboard.retry
        }
    }

    private func reset() {
        gesture = .none
        startedAt = nil
        stoppedAt = nil
    }
}
