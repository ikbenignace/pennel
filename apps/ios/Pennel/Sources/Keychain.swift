import Foundation
import Security

/// Provider API keys stay on this device: `AfterFirstUnlockThisDeviceOnly` keeps them out of backups and readable
/// while a recording runs with the screen locked.
enum Keychain {
    static let service = "app.pennel.ios"
    private static let apiKeyPrefix = "api-key."

    static func apiKeyAccount(_ providerId: String) -> String {
        apiKeyPrefix + providerId
    }

    static func apiKeys() -> ApiKeys {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecReturnAttributes as String: true,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitAll,
        ]
        var result: CFTypeRef?
        guard SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess,
              let items = result as? [[String: Any]]
        else { return ApiKeys(byProvider: [:]) }
        var keys: [String: String] = [:]
        for item in items {
            guard let account = item[kSecAttrAccount as String] as? String, account.hasPrefix(apiKeyPrefix),
                  let data = item[kSecValueData as String] as? Data, let key = String(data: data, encoding: .utf8)
            else { continue }
            keys[String(account.dropFirst(apiKeyPrefix.count))] = key
        }
        return ApiKeys(byProvider: keys)
    }

    static func hasKey(for providerId: String) -> Bool {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: apiKeyAccount(providerId),
        ]
        return SecItemCopyMatching(query as CFDictionary, nil) == errSecSuccess
    }

    /// An empty or nil key deletes the entry.
    @discardableResult
    static func set(key: String?, for providerId: String) -> Bool {
        guard !providerId.isEmpty else { return false }
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: apiKeyAccount(providerId),
        ]
        guard let key = key?.trimmingCharacters(in: .whitespacesAndNewlines), !key.isEmpty else {
            let status = SecItemDelete(query as CFDictionary)
            return status == errSecSuccess || status == errSecItemNotFound
        }
        let attributes: [String: Any] = [
            kSecValueData as String: Data(key.utf8),
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
        ]
        var status = SecItemUpdate(query as CFDictionary, attributes as CFDictionary)
        if status == errSecItemNotFound {
            status = SecItemAdd(query.merging(attributes) { $1 } as CFDictionary, nil)
        }
        return status == errSecSuccess
    }
}
