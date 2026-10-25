import PennelKit
import XCTest

final class PillPreviewTests: XCTestCase {
    func testCorrectionReplacesPreviousTextAndRejectsOldRevisions() {
        var state = PillPreviewState()
        state.begin("one")
        XCTAssertTrue(state.update(sessionID: "one", revision: 1, text: "Morgen om twee"))
        XCTAssertTrue(state.update(sessionID: "one", revision: 2, text: "Morgen om tien"))
        XCTAssertFalse(state.update(sessionID: "one", revision: 1, text: "Morgen om twee"))
        XCTAssertEqual(state.text, "Morgen om tien")
    }

    func testNewCaptureAndHidingRejectLateTextFromAnOldSession() {
        var state = PillPreviewState()
        state.begin("one")
        state.update(sessionID: "one", revision: 1, text: "Een eerste dictaat")
        state.begin("two")
        XCTAssertEqual(state.text, "")
        XCTAssertFalse(state.update(sessionID: "one", revision: 2, text: "Een laat resultaat"))
        state.clear(sessionID: "one")
        XCTAssertEqual(state.sessionID, "two")
        state.clear()
        XCTAssertFalse(state.update(sessionID: "two", revision: 1, text: "Nog een laat resultaat"))
        XCTAssertNil(state.sessionID)
    }

    func testRecentTailPreservesGraphemeClustersAndNormalizesWhitespace() {
        var state = PillPreviewState()
        state.begin("one")
        let family = "👨‍👩‍👧‍👦"
        state.update(sessionID: "one", revision: 1, text: String(repeating: "x", count: ThemePill.previewLimit) + family)
        XCTAssertEqual(state.text, "…" + String(repeating: "x", count: ThemePill.previewLimit - 1) + family)
        state.update(sessionID: "one", revision: 2, text: "  <b>Dit is tekst</b>\n\tgeen opmaak ")
        XCTAssertEqual(state.text, "<b>Dit is tekst</b> geen opmaak")
    }
}
