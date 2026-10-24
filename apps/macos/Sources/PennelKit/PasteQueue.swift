import Foundation

/// Releases results in the order their recordings started, even when they finish out of order.
public final class PasteQueue<T> {
    private var nextReservation = 0
    private var nextRelease = 0
    private var pending: [Int: T?] = [:]

    public init() {}

    public func reserve() -> Int {
        defer { nextReservation += 1 }
        return nextReservation
    }

    public func fulfill(_ seq: Int, with value: T?) -> [T] {
        pending[seq] = .some(value)
        var ready: [T] = []
        while let entry = pending.removeValue(forKey: nextRelease) {
            if let value = entry { ready.append(value) }
            nextRelease += 1
        }
        return ready
    }
}
