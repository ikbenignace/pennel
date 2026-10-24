struct RecordShortcutPreference {
    private(set) var enabled = false

    mutating func update(_ requested: Bool, start: () -> Bool, stop: () -> Void) -> Bool {
        if requested == enabled { return true }
        if requested {
            guard start() else { return false }
            enabled = true
        } else {
            enabled = false
            stop()
        }
        return true
    }
}
