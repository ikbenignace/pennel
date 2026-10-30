import Foundation
import LocalAuthentication
import Security

enum CredentialPresence {
    static func query(service: String, account: String, context: LAContext) -> [String: Any] {
        context.interactionNotAllowed = true
        return [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnAttributes as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
            kSecUseAuthenticationContext as String: context,
        ]
    }

    static func classify(_ status: OSStatus) -> Int32 {
        if status == errSecSuccess { return 1 }
        if status == errSecItemNotFound { return 0 }
        return status < 0 ? status : -1
    }

    static func lookup(service: String, account: String) -> Int32 {
        var keychain: SecKeychain?
        let status = SecKeychainCopyDomainDefault(.user, &keychain)
        guard status == errSecSuccess, let keychain else { return classify(status == errSecSuccess ? -1 : status) }
        let context = LAContext()
        var attributes = query(service: service, account: account, context: context)
        attributes[kSecMatchSearchList as String] = [keychain]
        var result: CFTypeRef?
        return classify(SecItemCopyMatching(attributes as CFDictionary, &result))
    }
}

@_cdecl("pennel_mac_credential_presence")
public func credentialPresence(_ rawService: UnsafePointer<CChar>?, _ rawAccount: UnsafePointer<CChar>?) -> Int32 {
    guard let service = decodeString(rawService), !service.isEmpty,
          let account = decodeString(rawAccount), !account.isEmpty else { return -1 }
    return CredentialPresence.lookup(service: service, account: account)
}
