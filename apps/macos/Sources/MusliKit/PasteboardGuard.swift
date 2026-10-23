import Foundation

public protocol PasteboardLike: AnyObject {
    var changeCount: Int { get }
    func snapshot() -> [[String: Data]]
    func restore(_ items: [[String: Data]])
    func setString(_ s: String)
}

public enum PasteboardGuard {
    /// Apps read the pasteboard whenever they handle Cmd-V; busy Electron apps can take several hundred ms.
    public static let restoreDelay: Duration = .milliseconds(700)

    public struct Token: Sendable {
        let saved: [[String: Data]]
        let changeCountAfterSet: Int
    }

    public static func begin(_ pasteboard: PasteboardLike, text: String) -> Token {
        let saved = pasteboard.snapshot()
        pasteboard.setString(text)
        return Token(saved: saved, changeCountAfterSet: pasteboard.changeCount)
    }

    /// Restores the user's clipboard unless they copied something new in the meantime.
    public static func end(_ pasteboard: PasteboardLike, _ token: Token) {
        guard pasteboard.changeCount == token.changeCountAfterSet else { return }
        pasteboard.restore(token.saved)
    }
}
