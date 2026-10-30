import LocalAuthentication
import Security
import XCTest
@testable import PennelMacPlatform

final class CredentialPresenceTests: XCTestCase {
    func testPresenceQueryRequestsMetadataWithoutSecretOrInteraction() {
        let context = LAContext()
        let query = CredentialPresence.query(service: "synthetic-service", account: "synthetic-account", context: context)
        XCTAssertEqual(query[kSecClass as String] as? String, kSecClassGenericPassword as String)
        XCTAssertEqual(query[kSecAttrService as String] as? String, "synthetic-service")
        XCTAssertEqual(query[kSecAttrAccount as String] as? String, "synthetic-account")
        XCTAssertEqual(query[kSecReturnAttributes as String] as? Bool, true)
        XCTAssertNil(query[kSecReturnData as String])
        XCTAssertTrue(context.interactionNotAllowed)
    }
    func testOnlyNotFoundMeansMissing() {
        XCTAssertEqual(CredentialPresence.classify(errSecSuccess), 1)
        XCTAssertEqual(CredentialPresence.classify(errSecItemNotFound), 0)
        XCTAssertLessThan(CredentialPresence.classify(errSecInteractionNotAllowed), 0)
        XCTAssertLessThan(CredentialPresence.classify(errSecAuthFailed), 0)
    }
}
