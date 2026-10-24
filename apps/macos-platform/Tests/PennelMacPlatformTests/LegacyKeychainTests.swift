import Foundation
import Security
import XCTest
@testable import PennelMacPlatform

final class LegacyKeychainTests: XCTestCase {
    private let legacy = Data("synthetic-old-key".utf8)

    func testExistingCanonicalEntryIsNeverReadFromLegacyOrReplaced() throws {
        var legacyReads = 0
        var writes = 0
        let copied = try LegacyKeyMigration.migrate(
            readCurrent: { KeychainRead(status: errSecSuccess, data: Data("synthetic-current".utf8)) },
            readLegacy: { legacyReads += 1; return KeychainRead(status: errSecSuccess, data: self.legacy) },
            addCurrent: { _ in writes += 1; return errSecSuccess }
        )
        XCTAssertFalse(copied)
        XCTAssertEqual(legacyReads, 0)
        XCTAssertEqual(writes, 0)
    }

    func testOnlyMissingCanonicalEntryAllowsImport() throws {
        var added: Data?
        let copied = try LegacyKeyMigration.migrate(
            readCurrent: { KeychainRead(status: errSecItemNotFound, data: nil) },
            readLegacy: { KeychainRead(status: errSecSuccess, data: self.legacy) },
            addCurrent: { added = $0; return errSecSuccess }
        )
        XCTAssertTrue(copied)
        XCTAssertEqual(added, legacy)
    }

    func testLockedOrDeniedCanonicalEntryDoesNotFallBackToLegacy() {
        for status in [errSecInteractionNotAllowed, errSecAuthFailed, errSecUserCanceled] {
            var legacyReads = 0
            XCTAssertThrowsError(try LegacyKeyMigration.migrate(
                readCurrent: { KeychainRead(status: status, data: nil) },
                readLegacy: { legacyReads += 1; return KeychainRead(status: errSecSuccess, data: self.legacy) },
                addCurrent: { _ in XCTFail("unexpected keychain write"); return errSecSuccess }
            ))
            XCTAssertEqual(legacyReads, 0)
        }
    }

    func testDuplicateAddRacePreservesTheCanonicalWinner() throws {
        let copied = try LegacyKeyMigration.migrate(
            readCurrent: { KeychainRead(status: errSecItemNotFound, data: nil) },
            readLegacy: { KeychainRead(status: errSecSuccess, data: self.legacy) },
            addCurrent: { _ in errSecDuplicateItem }
        )
        XCTAssertFalse(copied)
    }

    func testMissingLegacyEntryIsANoOp() throws {
        XCTAssertFalse(try LegacyKeyMigration.migrate(
            readCurrent: { KeychainRead(status: errSecItemNotFound, data: nil) },
            readLegacy: { KeychainRead(status: errSecItemNotFound, data: nil) },
            addCurrent: { _ in XCTFail("unexpected keychain write"); return errSecSuccess }
        ))
    }

    func testLegacyAccessAndAddFailuresArePropagated() {
        XCTAssertThrowsError(try LegacyKeyMigration.migrate(
            readCurrent: { KeychainRead(status: errSecItemNotFound, data: nil) },
            readLegacy: { KeychainRead(status: errSecInteractionNotAllowed, data: nil) },
            addCurrent: { _ in XCTFail("unexpected keychain write"); return errSecSuccess }
        ))
        XCTAssertThrowsError(try LegacyKeyMigration.migrate(
            readCurrent: { KeychainRead(status: errSecItemNotFound, data: nil) },
            readLegacy: { KeychainRead(status: errSecSuccess, data: self.legacy) },
            addCurrent: { _ in errSecAuthFailed }
        ))
    }
}
