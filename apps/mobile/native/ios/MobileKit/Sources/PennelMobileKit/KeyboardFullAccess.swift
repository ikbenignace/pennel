import Foundation

/// Set by the keyboard the first time it runs with Full Access, so the app can tell the user the keyboard is ready.
/// It is one boolean in the App Group defaults; nothing typed or dictated is ever stored with it. Without Full Access
/// the keyboard cannot reach the group, so the flag is never cleared from there.
public enum KeyboardFullAccess {
    public static let defaultsKey = "keyboardFullAccessSeen"

    public static func markSeen(in defaults: UserDefaults?) {
        guard let defaults, !defaults.bool(forKey: defaultsKey) else { return }
        defaults.set(true, forKey: defaultsKey)
    }

    public static func seen(in defaults: UserDefaults?) -> Bool {
        defaults?.bool(forKey: defaultsKey) ?? false
    }
}
