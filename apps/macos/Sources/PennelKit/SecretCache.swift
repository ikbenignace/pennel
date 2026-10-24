import Foundation

/// Reads each secret at most once per launch. Every Keychain read by a self-signed build can make
/// macOS ask for the login password, so repeated reads (per dictation, per redraw) must not hit it.
public final class SecretCache: @unchecked Sendable {
    private let load: (String) -> String?
    private let lock = NSLock()
    private var values: [String: String?] = [:]

    public init(load: @escaping (String) -> String?) {
        self.load = load
    }

    public func get(_ account: String) -> String? {
        if let cached = lock.withLock({ values[account] }) { return cached }
        let value = load(account)
        lock.withLock { values[account] = .some(value) }
        return value
    }

    public func store(_ value: String?, for account: String) {
        lock.withLock { values[account] = .some(value) }
    }
}
