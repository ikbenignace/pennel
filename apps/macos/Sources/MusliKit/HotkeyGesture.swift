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

    public init(tapMax: TimeInterval = 0.25, doubleTapWindow: TimeInterval = 0.35, maxDuration: TimeInterval = 600) {
        self.tapMax = tapMax
        self.doubleTapWindow = doubleTapWindow
        self.maxDuration = maxDuration
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

    public mutating func tick(at t: TimeInterval) -> [GestureAction] {
        switch phase {
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
