import Foundation
import XCTest
@testable import PennelMacPlatform

final class ExternalPasteTargetTests: XCTestCase {
    private let target = ExternalPasteIdentity(pid: 42, launched: Date(timeIntervalSince1970: 100), bundleID: "example.editor")

    func testOwnWindowActivationDoesNotReplaceTheCapturedDestination() {
        var guardState = ExternalPasteGuard(identity: target)
        guardState.observeActivation(pid: 99, ownPID: 99)
        XCTAssertTrue(guardState.accepts(target))
        guardState.observeActivation(pid: target.pid, ownPID: 99)
        XCTAssertTrue(guardState.accepts(target))
    }

    func testThirdAppActivationCancelsEvenWhenTheUserReturns() {
        var guardState = ExternalPasteGuard(identity: target)
        guardState.observeActivation(pid: 43, ownPID: 99)
        guardState.observeActivation(pid: target.pid, ownPID: 99)
        XCTAssertFalse(guardState.accepts(target))
    }

    func testRestartedAppWithTheSamePIDAndBundleIsADifferentTarget() {
        let guardState = ExternalPasteGuard(identity: target)
        XCTAssertFalse(guardState.accepts(ExternalPasteIdentity(pid: 42, launched: Date(timeIntervalSince1970: 200), bundleID: "example.editor")))
        XCTAssertFalse(guardState.accepts(ExternalPasteIdentity(pid: 42, launched: target.launched, bundleID: "example.other")))
        XCTAssertTrue(guardState.accepts(target))
    }

    func testGlobalDictationNeverReactivatesItsTargetAfterPennelTakesFocus() {
        var guardState = ExternalPasteGuard(identity: target, currentOnly: true)
        XCTAssertTrue(guardState.canPaste(foregroundPID: target.pid, ownPID: 99))
        XCTAssertFalse(guardState.canPaste(foregroundPID: 99, ownPID: 99))
        guardState.observeActivation(pid: 99, ownPID: 99)
        guardState.observeActivation(pid: target.pid, ownPID: 99)
        XCTAssertFalse(guardState.accepts(target))
    }

    func testPadDeliveryCanRestoreItsCapturedAppButNeverAThirdApp() {
        let guardState = ExternalPasteGuard(identity: target)
        XCTAssertTrue(guardState.canPaste(foregroundPID: 99, ownPID: 99))
        XCTAssertTrue(guardState.canPaste(foregroundPID: target.pid, ownPID: 99))
        XCTAssertFalse(guardState.canPaste(foregroundPID: 43, ownPID: 99))
        XCTAssertFalse(guardState.canPaste(foregroundPID: nil, ownPID: 99))
    }
}
