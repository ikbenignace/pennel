import Foundation

public struct PillPreviewState: Sendable {
    public private(set) var sessionID: String?
    public private(set) var revision: UInt64 = 0
    public private(set) var text = ""

    public init() {}

    public mutating func begin(_ sessionID: String) {
        guard !sessionID.isEmpty else { return }
        self.sessionID = sessionID
        revision = 0
        text = ""
    }

    @discardableResult
    public mutating func update(sessionID: String, revision: UInt64, text: String) -> Bool {
        guard self.sessionID == sessionID, revision > self.revision else { return false }
        self.revision = revision
        let normalized = text.split(whereSeparator: \.isWhitespace).joined(separator: " ")
        self.text = normalized.count > ThemePill.previewLimit ? "…" + normalized.suffix(ThemePill.previewLimit) : normalized
        return true
    }

    public mutating func clear(sessionID: String? = nil) {
        guard sessionID == nil || sessionID == self.sessionID else { return }
        self.sessionID = nil
        revision = 0
        text = ""
    }
}
