import Foundation

/// The letter layout of the Pennel keyboard. Stored as a plain name in the App Group defaults, never as typed text.
public enum KeyboardLayout: String, CaseIterable, Sendable {
    case qwerty, azerty

    public static let defaultsKey = "keyboardLayout"

    public var rows: [[String]] {
        switch self {
        case .qwerty: ["qwertyuiop", "asdfghjkl", "zxcvbnm"].map { $0.map(String.init) }
        case .azerty: ["azertyuiop", "qsdfghjklm", "wxcvbn"].map { $0.map(String.init) }
        }
    }

    public static func stored(in defaults: UserDefaults?) -> KeyboardLayout {
        defaults?.string(forKey: defaultsKey).flatMap(KeyboardLayout.init(rawValue:)) ?? .qwerty
    }

    public func store(in defaults: UserDefaults?) {
        defaults?.set(rawValue, forKey: Self.defaultsKey)
    }

    public static var groupDefaults: UserDefaults? { UserDefaults(suiteName: HandoffStore.appGroup) }
}
