import Foundation
@testable import Pennel
import XCTest

final class WorkspaceStateTests: XCTestCase {
    func testOnboardingWindowFitsShortLogicalWorkArea() {
        let screen = NSRect(x: 0, y: 0, width: 800, height: 500)
        let result = WindowLayout.constrained(size: NSSize(width: 850, height: 690), to: screen)
        XCTAssertEqual(result, NSSize(width: 752, height: 452))
        let minimum = WindowLayout.constrained(size: NSSize(width: 580, height: 540), to: screen)
        XCTAssertLessThanOrEqual(minimum.height, result.height)
    }

    func testWindowMinimumDoesNotExceedTinyWorkArea() {
        let screen = NSRect(x: 0, y: 0, width: 420, height: 320)
        let result = WindowLayout.constrained(size: NSSize(width: 580, height: 540), to: screen)
        XCTAssertEqual(result, NSSize(width: 372, height: 272))
    }

    @MainActor
    func testHistoryLabelsUseReadableSharedApplicationNames() async {
        XCTAssertEqual(AppIcon.name(for: "com.apple.mail"), "Mail")
        XCTAssertEqual(AppIcon.name(for: "dev.pennel.Pennel.native.preview"), "Pennel")
        XCTAssertEqual(AppIcon.name(for: "com.example.editor"), "Editor")
        XCTAssertEqual(AppIcon.name(for: "notepad.exe"), "Notepad")
    }

    @MainActor
    func testBackgroundRecordingCompletionDoesNotStealSettingsNavigation() async {
        let state = WorkspaceState()
        state.destination = .settings
        state.showingRecording = true
        state.recordingFinished("new-meeting")
        XCTAssertEqual(state.destination, .settings)
        XCTAssertNil(state.meetingSelection)
        XCTAssertFalse(state.showingRecording)
    }

    @MainActor
    func testVisibleRecordingCompletionShowsItsOwnMeeting() async {
        let state = WorkspaceState()
        state.destination = .meetings
        state.showingRecording = true
        state.recordingFinished("new-meeting")
        XCTAssertEqual(state.destination, .meetings)
        XCTAssertEqual(state.meetingSelection, "new-meeting")
        XCTAssertFalse(state.showingRecording)
    }

    @MainActor
    func testFailedDraftSavePreservesEditsAndBlocksCapturePreflight() async {
        let state = WorkspaceState()
        state.attachPad(text: "Bewaard") { _ in throw CocoaError(.fileWriteNoPermission) }
        state.editPad("Nieuwe handmatige wijzigingen")
        XCTAssertFalse(state.flushPad())
        XCTAssertEqual(state.padText, "Nieuwe handmatige wijzigingen")
        XCTAssertNotNil(state.padError)
    }

    @MainActor
    func testDraftCanRecoverAfterStorageBecomesWritable() async {
        let state = WorkspaceState()
        var stored = "Bewaard"
        var writable = false
        state.attachPad(text: stored) { text in
            guard writable else { throw CocoaError(.fileWriteNoPermission) }
            stored = text
        }
        state.editPad("Een aangepast concept")
        XCTAssertFalse(state.flushPad())
        writable = true
        XCTAssertTrue(state.flushPad())
        XCTAssertEqual(stored, "Een aangepast concept")
        XCTAssertNil(state.padError)
    }

    @MainActor
    func testNavigationRetainsDraftsAndMeetingTabs() async {
        let state = WorkspaceState()
        state.attachPad(text: "Concept") { _ in }
        state.chatDrafts["meeting"] = "Mijn ongestelde vraag"
        state.meetingTabs["meeting"] = .transcript
        state.templateEditing = TemplatesSection.EditorTarget(id: nil, name: "Eigen sjabloon", body: "Een nog niet bewaard sjabloon")
        state.historyRetrying.insert("pending")
        state.selectMeeting("meeting")
        state.destination = .settings
        state.destination = .dictation
        XCTAssertEqual(state.padText, "Concept")
        XCTAssertEqual(state.chatDrafts["meeting"], "Mijn ongestelde vraag")
        XCTAssertEqual(state.meetingTabs["meeting"], .transcript)
        XCTAssertEqual(state.meetingSelection, "meeting")
        XCTAssertEqual(state.templateEditing?.body, "Een nog niet bewaard sjabloon")
        XCTAssertTrue(state.historyRetrying.contains("pending"))
    }

    @MainActor
    func testRecognizedTextCanRemainEditableWhenStorageFails() async {
        let state = WorkspaceState()
        var stored = "Handmatig bewerkt"
        var writable = false
        state.attachPad(text: stored) { value in
            guard writable else { throw CocoaError(.fileWriteNoPermission) }
            stored = value
        }
        state.retainUnsavedPad("Handmatig bewerkt en nieuw gedicteerd")
        XCTAssertEqual(state.padText, "Handmatig bewerkt en nieuw gedicteerd")
        XCTAssertFalse(state.flushPad())
        writable = true
        XCTAssertTrue(state.flushPad())
        XCTAssertEqual(stored, "Handmatig bewerkt en nieuw gedicteerd")
    }

    @MainActor
    func testMeetingDraftFailureRetainsOnlyUnsavedChanges() async {
        let state = WorkspaceState()
        state.meetingNoteDrafts = ["one": "Eerste notitie", "two": "Bewerkte notitie"]
        XCTAssertFalse(state.flushMeetingNotes { id, _ in if id == "two" { throw CocoaError(.fileWriteNoPermission) } })
        XCTAssertNil(state.meetingNoteDrafts["one"])
        XCTAssertEqual(state.meetingNoteDrafts["two"], "Bewerkte notitie")
        XCTAssertTrue(state.flushMeetingNotes { _, _ in })
        XCTAssertTrue(state.meetingNoteDrafts.isEmpty)
    }
}
