import Foundation
import PennelKit
import Security

enum Keychain {
    private static var service: String {
        let bundle = Bundle.main.bundleIdentifier ?? "dev.pennel.Pennel"
        return bundle.hasSuffix(".preview") ? bundle : "dev.pennel.Pennel"
    }
    private static let cache = SecretCache(load: read)

    static func get(_ account: String) -> String? {
        guard !account.isEmpty else { return nil }
        return cache.get(account)
    }

    /// Keys saved before the rename to Pennel live under the old service name.
    static func legacyValue(_ account: String) -> String? {
        guard !service.hasSuffix(".preview") else { return nil }
        return read(account, service: "dev.musli.Musli")
    }

    private static func read(_ account: String) -> String? {
        read(account, service: service)
    }

    private static func read(_ account: String, service: String) -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var item: CFTypeRef?
        guard SecItemCopyMatching(query as CFDictionary, &item) == errSecSuccess,
              let data = item as? Data
        else { return nil }
        return String(data: data, encoding: .utf8)
    }

    @discardableResult
    static func set(_ value: String?, for account: String) -> Bool {
        guard !account.isEmpty else { return false }
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
        let value = value.flatMap { $0.isEmpty ? nil : $0 }
        guard let value else {
            let status = SecItemDelete(query as CFDictionary)
            guard status == errSecSuccess || status == errSecItemNotFound else { return false }
            cache.store(nil, for: account)
            return true
        }
        let data: [String: Any] = [kSecValueData as String: Data(value.utf8)]
        var status = SecItemUpdate(query as CFDictionary, data as CFDictionary)
        if status == errSecItemNotFound {
            var attributes = query
            attributes[kSecValueData as String] = Data(value.utf8)
            attributes[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
            status = SecItemAdd(attributes as CFDictionary, nil)
        }
        guard status == errSecSuccess else { return false }
        cache.store(value, for: account)
        return true
    }
}
