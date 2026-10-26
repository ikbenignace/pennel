import Foundation
import Security
import Testing
@testable import Pennel

struct KeychainTests {
    @Test func keychain_roundtrip_and_accessibility() throws {
        let provider = "test-\(UUID().uuidString)"
        defer { Keychain.set(key: nil, for: provider) }

        #expect(Keychain.set(key: "sk-first", for: provider))
        #expect(Keychain.set(key: "sk-second", for: provider))
        #expect(Keychain.apiKeys().byProvider[provider] == "sk-second")

        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: Keychain.service,
            kSecAttrAccount as String: Keychain.apiKeyAccount(provider),
            kSecReturnAttributes as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var item: CFTypeRef?
        #expect(SecItemCopyMatching(query as CFDictionary, &item) == errSecSuccess)
        let attributes = try #require(item as? [String: Any])
        #expect(attributes[kSecAttrAccessible as String] as? String
            == kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly as String)
        #expect(Keychain.service == "app.pennel.ios")

        #expect(Keychain.set(key: nil, for: provider))
        #expect(Keychain.apiKeys().byProvider[provider] == nil)
    }
}
