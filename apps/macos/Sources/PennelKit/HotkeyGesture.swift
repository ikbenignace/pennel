import Foundation

public enum KeyEdge: Sendable {
    case down, up
}

public enum GestureAction: Equatable, Sendable {
    case start, submit, cancel
}

/// Turns raw key edges into dictation actions: hold to talk, double-tap for hands-free.
public struct HotkeyGesture: Sendable {
    private enum Phase {
        case idle
        case manual(startedAt: TimeInterval)
        case holding(pressedAt: TimeInterval)
        case handsFree(startedAt: TimeInterval, awaitingRelease: Bool)
        case awaitingRelease
    }

    private let tapMax: TimeInterval
    private let doubleTapWindow: TimeInterval
    private let maxDuration: TimeInterval
    private var phase: Phase = .idle
    private var lastTapUp: TimeInterval?
    public private(set) var isHandsFree = false
    public var isManual: Bool { if case .manual = phase { true } else { false } }

    public init(tapMax: TimeInterval = 0.25, doubleTapWindow: TimeInterval = 0.35, maxDuration: TimeInterval = 600) {
        self.tapMax = tapMax
        self.doubleTapWindow = doubleTapWindow
        self.maxDuration = maxDuration
    }

    /// Explicit UI recording does not depend on observing a physical modifier release.
    public mutating func manualToggle(at t: TimeInterval) -> [GestureAction] {
        lastTapUp = nil
        switch phase {
        case .idle, .awaitingRelease:
            phase = .manual(startedAt: t)
            isHandsFree = true
            return [.start]
        case .holding, .handsFree(_, awaitingRelease: true):
            phase = .awaitingRelease
            isHandsFree = false
            return [.submit]
        case .manual, .handsFree(_, awaitingRelease: false):
            phase = .idle
            isHandsFree = false
            return [.submit]
        }
    }

    public mutating func handle(_ edge: KeyEdge, at t: TimeInterval) -> [GestureAction] {
        switch (edge, phase) {
        case (.down, .idle):
            if let tapUp = lastTapUp, t - tapUp <= doubleTapWindow {
                lastTapUp = nil
                isHandsFree = true
                phase = .handsFree(startedAt: t, awaitingRelease: true)
            } else {
                phase = .holding(pressedAt: t)
            }
            return [.start]
        case (.down, .handsFree(_, awaitingRelease: false)):
            isHandsFree = false
            phase = .awaitingRelease
            return [.submit]
        case (.up, .holding(let pressedAt)):
            phase = .idle
            if t - pressedAt >= tapMax {
                return [.submit]
            }
            lastTapUp = t
            return [.cancel]
        case (.up, .handsFree(let startedAt, awaitingRelease: true)):
            phase = .handsFree(startedAt: startedAt, awaitingRelease: false)
            return []
        case (.up, .awaitingRelease):
            phase = .idle
            return []
        default:
            return []
        }
    }

    /// Another key pressed while holding means the hotkey is used as a modifier (fn+arrow, ⌘+C), not to dictate.
    public mutating func interrupt() -> [GestureAction] {
        guard case .holding = phase else { return [] }
        phase = .awaitingRelease
        return [.cancel]
    }

    public mutating func tick(at t: TimeInterval) -> [GestureAction] {
        switch phase {
        case .manual(let startedAt) where t - startedAt >= maxDuration:
            isHandsFree = false
            phase = .idle
        case .holding(let startedAt) where t - startedAt >= maxDuration:
            phase = .awaitingRelease
        case .handsFree(let startedAt, let awaitingRelease) where t - startedAt >= maxDuration:
            isHandsFree = false
            phase = awaitingRelease ? .awaitingRelease : .idle
        default:
            return []
        }
        return [.submit]
    }
}
