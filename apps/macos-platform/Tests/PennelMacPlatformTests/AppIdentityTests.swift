import XCTest
@testable import PennelMacPlatform

final class AppIdentityTests: XCTestCase {
    func testInstalledAppReturnsNameAndPNGIcon() throws {
        let identity = try XCTUnwrap(appIdentity(bundleID: "com.apple.finder", points: 20))
        XCTAssertFalse(identity.name.isEmpty)
        XCTAssertTrue(identity.iconDataURL.hasPrefix("data:image/png;base64,"))
    }

    func testUnknownOrInvalidAppHasNoIdentity() {
        XCTAssertNil(appIdentity(bundleID: "dev.pennel.does-not-exist", points: 20))
        XCTAssertNil(appIdentity(bundleID: "", points: 20))
    }
}
