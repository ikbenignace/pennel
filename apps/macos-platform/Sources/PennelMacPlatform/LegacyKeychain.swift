import Foundation
import Security

struct KeychainRead {
    let status: OSStatus
    let data: Data?
}

struct KeychainMigrationError: LocalizedError {
    let status: OSStatus
    var errorDescription: String? {
        let reason = SecCopyErrorMessageString(status, nil) as String? ?? "Unknown error"
        return "Keychain migration failed (\(status)): \(reason)"
    }
}

enum LegacyKeyMigration {
    static func migrate(
        readCurrent: () -> KeychainRead,
        readLegacy: () -> KeychainRead,
        addCurrent: (Data) -> OSStatus
    ) throws -> Bool {
        let current = readCurrent()
        if current.status == errSecSuccess { return false }
        guard current.status == errSecItemNotFound else { throw KeychainMigrationError(status: current.status) }
        let legacy = readLegacy()
        if legacy.status == errSecItemNotFound { return false }
        guard legacy.status == errSecSuccess else { throw KeychainMigrationError(status: legacy.status) }
        guard let data = legacy.data, !data.isEmpty, String(data: data, encoding: .utf8) != nil else {
            throw KeychainMigrationError(status: errSecDecode)
        }
        // A concurrent canonical write wins: SecItemAdd is atomic and never replaces an item.
        let result = addCurrent(data)
        if result == errSecDuplicateItem { return false }
        guard result == errSecSuccess else { throw KeychainMigrationError(status: result) }
        return true
    }

    static func migrate(account: String) throws -> Bool {
        func query(service: String) -> [String: Any] {
            [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrService as String: service,
                kSecAttrAccount as String: account,
            ]
        }
        func read(service: String) -> KeychainRead {
            var attributes = query(service: service)
            attributes[kSecReturnData as String] = true
            attributes[kSecMatchLimit as String] = kSecMatchLimitOne
            var item: CFTypeRef?
            let status = SecItemCopyMatching(attributes as CFDictionary, &item)
            return KeychainRead(status: status, data: item as? Data)
        }
        return try migrate(
            readCurrent: { read(service: "dev.pennel.Pennel") },
            readLegacy: { read(service: "dev.musli.Musli") },
            addCurrent: { data in
                var attributes = query(service: "dev.pennel.Pennel")
                attributes[kSecValueData as String] = data
                attributes[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
                return SecItemAdd(attributes as CFDictionary, nil)
            }
        )
    }
}

@_cdecl("pennel_mac_migrate_legacy_key")
public func migrateLegacyKey(_ rawAccount: UnsafePointer<CChar>?, _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?) -> Bool {
    guard let account = decodeString(rawAccount), !account.isEmpty else {
        errorOut?.pointee = bridgeString("No provider account specified")
        return false
    }
    do { return try LegacyKeyMigration.migrate(account: account) }
    catch { errorOut?.pointee = bridgeString(error.localizedDescription); return false }
}
