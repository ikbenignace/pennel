/// Whether the current recording is a spoken command: ⇧ pressed at any point while the dictation key is held.
public struct CommandIntent: Sendable {
    public private(set) var isCommand = false

    public init() {}

    public mutating func shift(_ down: Bool, keyHeld: Bool) {
        if down && keyHeld { isCommand = true }
    }

    public mutating func reset() {
        isCommand = false
    }
}
