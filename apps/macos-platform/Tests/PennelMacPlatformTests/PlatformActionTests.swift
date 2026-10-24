import Foundation
import XCTest
@testable import PennelMacPlatform

final class PlatformActionTests: XCTestCase {
    func testRecordingShortcutStartsDisabledAndStopsWhenDisabledAgain() {
        var preference = RecordShortcutPreference()
        var starts = 0
        var stops = 0
        XCTAssertFalse(preference.enabled)
        XCTAssertTrue(preference.update(false, start: { starts += 1; return true }, stop: { stops += 1 }))
        XCTAssertEqual(starts, 0)
        XCTAssertEqual(stops, 0)
        XCTAssertTrue(preference.update(true, start: { starts += 1; return true }, stop: { stops += 1 }))
        XCTAssertTrue(preference.enabled)
        XCTAssertTrue(preference.update(true, start: { starts += 1; return true }, stop: { stops += 1 }))
        XCTAssertEqual(starts, 1)
        XCTAssertTrue(preference.update(false, start: { starts += 1; return true }, stop: { stops += 1 }))
        XCTAssertFalse(preference.enabled)
        XCTAssertEqual(stops, 1)
    }

    func testFailedShortcutRegistrationDoesNotMarkItEnabled() {
        var preference = RecordShortcutPreference()
        XCTAssertFalse(preference.update(true, start: { false }, stop: { XCTFail("unexpected stop") }))
        XCTAssertFalse(preference.enabled)
    }

    func testMailComposerURLPreservesSubjectAndBodyAsData() throws {
        let title = "Meeting & planning? 📌"
        let text = "Notes\nrecipient=example.invalid&subject=other"
        let url = try XCTUnwrap(composeMailURL(text: text, title: title))
        let components = try XCTUnwrap(URLComponents(url: url, resolvingAgainstBaseURL: false))
        XCTAssertEqual(components.scheme, "mailto")
        XCTAssertEqual(components.queryItems?.first(where: { $0.name == "subject" })?.value, title)
        XCTAssertEqual(components.queryItems?.first(where: { $0.name == "body" })?.value, text)
        XCTAssertEqual(components.queryItems?.count, 2)
    }

    func testUnknownShareActionDoesNotOpenAnExternalApp() throws {
        let error = "sample".withCString { text in
            "meeting".withCString { title in
                "invalid".withCString { action in shareNative(text, title, action) }
            }
        }
        let pointer = try XCTUnwrap(error)
        defer { stringFree(pointer) }
        XCTAssertEqual(String(cString: pointer), "Onbekende deelactie")
    }
}
