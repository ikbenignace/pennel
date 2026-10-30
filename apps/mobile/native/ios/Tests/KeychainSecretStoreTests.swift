import Foundation
import Security
import Testing
@testable internal import PennelMobile

struct KeychainSecretStoreTests {
    @Test func secret_store_roundtrip_keychain() throws {
        let store = KeychainSecretStore()
        let account = "test-\(UUID().uuidString)"
        defer { store.set(account: account, value: nil) }

        #expect(store.get(account: account) == nil)
        store.set(account: account, value: "first")
        store.set(account: account, value: "second")
        #expect(store.get(account: account) == "second")

        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: Keychain.service,
            kSecAttrAccount as String: account,
            kSecReturnAttributes as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var item: CFTypeRef?
        #expect(SecItemCopyMatching(query as CFDictionary, &item) == errSecSuccess)
        let attributes = try #require(item as? [String: Any])
        #expect(attributes[kSecAttrAccessible as String] as? String
            == kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly as String)

        store.set(account: account, value: nil)
        #expect(store.get(account: account) == nil)
    }

    @Test func cloud_account_names_match_the_desktop() {
        #expect(KeychainSecretStore.refreshAccount == "pennel-cloud-refresh")
        #expect(KeychainSecretStore.vaultKeyAccount == "pennel-cloud-vault-key")
    }
}
