import Foundation

public struct KeyboardInsertionContext: Equatable, Sendable {
    public let document: UUID
    public let lifetime: UUID
    public let generation: UInt64

    public init(document: UUID, lifetime: UUID, generation: UInt64) {
        self.document = document
        self.lifetime = lifetime
        self.generation = generation
    }
}

public struct KeyboardDeliveryGuard: Sendable {
    private var origin: KeyboardInsertionContext?
    private var stop: UUID?
    private var invalidated = false

    public init() {}

    public mutating func begin(in context: KeyboardInsertionContext?) {
        origin = context
        stop = nil
        invalidated = context == nil
    }

    public mutating func awaitResult(_ id: UUID) { stop = id }

    public mutating func invalidate() { invalidated = true }

    public func permits(_ id: UUID, in context: KeyboardInsertionContext?) -> Bool {
        !invalidated && stop == id && origin != nil && origin == context
    }

    public mutating func reset() {
        origin = nil
        stop = nil
        invalidated = false
    }
}
