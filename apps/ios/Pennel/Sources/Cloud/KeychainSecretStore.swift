import Foundation
import os

/// The core's credential store (refresh token, vault key) on the Keychain, so sync and hosted models keep working
/// with the screen locked and the secrets never leave this device.
final class KeychainSecretStore: SecretStore {
    static let refreshAccount = "pennel-cloud-refresh"
    static let vaultKeyAccount = "pennel-cloud-vault-key"
    private static let log = Logger(subsystem: "app.pennel.ios", category: "cloud")

    func get(account: String) -> String? {
        Keychain.secret(account: account)
    }

    func set(account: String, value: String?) {
        if !Keychain.write(value, account: account) {
            Self.log.error("secret not saved")
        }
    }
}
