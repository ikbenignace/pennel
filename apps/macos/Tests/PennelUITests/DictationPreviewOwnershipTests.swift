import Foundation
@testable import Pennel
import XCTest

final class DictationPreviewOwnershipTests: XCTestCase {
    func testUnknownAndFinishedRecordingsCannotUpdateThePill() {
        var ownership = DictationPreviewOwnership()
        XCTAssertFalse(ownership.accepts("unknown", currentSession: nil))
        ownership.register("recording", owner: nil)
        XCTAssertTrue(ownership.accepts("recording", currentSession: nil))
        ownership.remove("recording")
        XCTAssertFalse(ownership.accepts("recording", currentSession: nil))
    }

    func testClosingAnOwnedSessionRejectsItsQueuedPreviewImmediately() {
        var ownership = DictationPreviewOwnership()
        ownership.register("recording", owner: "practice")
        XCTAssertTrue(ownership.accepts("recording", currentSession: "practice"))
        XCTAssertFalse(ownership.accepts("recording", currentSession: nil))
    }

    func testAReplacementSessionNeverReceivesTheOldSessionsPreview() {
        var ownership = DictationPreviewOwnership()
        ownership.register("old-recording", owner: "old-practice")
        ownership.register("new-recording", owner: "new-practice")
        XCTAssertFalse(ownership.accepts("old-recording", currentSession: "new-practice"))
        XCTAssertTrue(ownership.accepts("new-recording", currentSession: "new-practice"))
    }

    func testOwnedCancellationDoesNotCancelAnIndependentExternalRecording() {
        var ownership = DictationPreviewOwnership()
        ownership.register("external", owner: nil)
        ownership.register("pad", owner: "pad-session")
        ownership.register("practice", owner: "practice-session")
        for id in ownership.recordings(ownedBy: "pad-session") { ownership.remove(id) }
        XCTAssertFalse(ownership.accepts("pad", currentSession: "pad-session"))
        XCTAssertTrue(ownership.accepts("external", currentSession: "practice-session"))
        XCTAssertTrue(ownership.accepts("practice", currentSession: "practice-session"))
    }
}

final class DictationSelectionSnapshotTests: XCTestCase {
    @MainActor
    func testChangedTargetNeverReadsTheCurrentFieldsSelection() async {
        var reads = 0
        let result = await DictationSelectionSnapshot.read(isCurrent: { false }, selection: {
            reads += 1
            return "A selection from the wrong field"
        })
        XCTAssertEqual(result, .changed)
        XCTAssertEqual(reads, 0)
    }

    @MainActor
    func testTargetChangeDuringSelectionReadDiscardsThatText() async {
        var current = true
        let result = await DictationSelectionSnapshot.read(isCurrent: { current }, selection: {
            current = false
            return "Text that must not reach a command provider"
        })
        XCTAssertEqual(result, .changed)
    }

    @MainActor
    func testAnUnchangedTargetWithoutASelectionCanStillReceiveACommand() async {
        let result = await DictationSelectionSnapshot.read(isCurrent: { true }, selection: { nil })
        XCTAssertEqual(result, .current(nil))
    }
}

final class DictationApplicationContextTests: XCTestCase {
    func testGlobalDictationUsesTheApplicationAtReleaseForItsStyle() {
        XCTAssertEqual(DictationApplicationContext.bundleID(
            owned: false, atStart: "com.apple.mail", atStop: "com.tinyspeck.slackmacgap"
        ), "com.tinyspeck.slackmacgap")
    }

    func testAnUnknownApplicationAtReleaseNeverReusesThePreviousApplicationsStyle() {
        XCTAssertNil(DictationApplicationContext.bundleID(
            owned: false, atStart: "com.apple.mail", atStop: nil
        ))
    }

    func testOwnedPadAndPracticeKeepPennelsContextWhenAnotherAppBecomesActive() {
        XCTAssertEqual(DictationApplicationContext.bundleID(
            owned: true, atStart: "dev.pennel.Pennel.native.preview", atStop: "com.apple.mail"
        ), "dev.pennel.Pennel.native.preview")
    }
}
