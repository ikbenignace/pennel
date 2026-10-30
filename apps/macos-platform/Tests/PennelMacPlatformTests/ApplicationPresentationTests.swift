import AppKit
import XCTest
@testable import PennelMacPlatform

final class ApplicationPresentationTests: XCTestCase {
    func testProductWindowsExposeMenusAndBackgroundOverlaysRemainAccessory() {
        XCTAssertEqual(productWindowActivationPolicy(visible: true), .regular)
        XCTAssertEqual(productWindowActivationPolicy(visible: false), .accessory)
    }
}
