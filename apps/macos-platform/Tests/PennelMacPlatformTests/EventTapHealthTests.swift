import CoreGraphics
import XCTest
@testable import PennelMacPlatform

final class EventTapHealthTests: XCTestCase {
    func testAuthorizationRequiredEvenWhenTheTapExists() {
        XCTAssertFalse(health(authorized: false).isAvailable)
    }

    func testPartialTapIsNotReportedAsAvailable() {
        let flagsOnly = CGEventMask(1 << CGEventType.flagsChanged.rawValue)
        XCTAssertFalse(health(events: flagsOnly).isAvailable)
        XCTAssertTrue(health().isAvailable)
    }

    func testRevokedAccessAndDisabledOrInvalidPortsRequireRecovery() {
        XCTAssertFalse(health(authorized: false).isAvailable)
        XCTAssertFalse(health(valid: false).isAvailable)
        XCTAssertFalse(health(enabled: false).isAvailable)
        XCTAssertFalse(health(events: nil).isAvailable)
        XCTAssertTrue(health().isAvailable)
    }

    func testInspectingANewTapUsesItsIdentityRatherThanAnotherHealthyTap() {
        let prior = tap(id: 1, events: EventTapHealth.requiredEvents)
        let partial = tap(id: 2, events: CGEventMask(1 << CGEventType.flagsChanged.rawValue))
        let unrelated = tap(id: 3, process: 202, events: EventTapHealth.requiredEvents)
        XCTAssertEqual(EventTapInspection.newTapID(previous: [1], current: [prior, partial, unrelated], process: 101), 2)
        XCTAssertNil(EventTapInspection.newTapID(previous: [1], current: [prior, unrelated], process: 101))
    }

    func testAmbiguousConcurrentTapsAreNotMistakenForTheRequestedTap() {
        let first = tap(id: 2, events: EventTapHealth.requiredEvents)
        let second = tap(id: 3, events: EventTapHealth.requiredEvents)
        XCTAssertNil(EventTapInspection.newTapID(previous: [], current: [first, second], process: 101))
    }

    func testPartialStartupIsRecreatedAfterAccessIsGranted() {
        let driver = FakeTap(authorized: false, events: CGEventMask(1 << CGEventType.flagsChanged.rawValue))
        XCTAssertFalse(driver.start())
        XCTAssertNil(driver.events)
        XCTAssertEqual(driver.creations, 0)
        driver.authorized = true
        XCTAssertTrue(driver.start())
        XCTAssertEqual(driver.events, EventTapHealth.requiredEvents)
        XCTAssertEqual(driver.creations, 1)
        XCTAssertTrue(driver.start())
        XCTAssertEqual(driver.creations, 1, "healthy taps must not be recreated every poll")
    }

    func testAccessLossAndRestorationRecoverWithoutRestartingTheMonitor() {
        let driver = FakeTap(authorized: true, events: EventTapHealth.requiredEvents)
        XCTAssertTrue(driver.start())
        driver.authorized = false
        XCTAssertFalse(driver.start())
        XCTAssertNil(driver.events)
        driver.authorized = true
        XCTAssertTrue(driver.start())
        XCTAssertEqual(driver.creations, 1)
    }

    func testCreationWithAStrippedMaskIsDisposedRatherThanKeptAsHealthy() {
        let driver = FakeTap(authorized: true, events: nil)
        driver.createdEvents = CGEventMask(1 << CGEventType.flagsChanged.rawValue)
        XCTAssertFalse(driver.start())
        XCTAssertNil(driver.events)
        driver.createdEvents = EventTapHealth.requiredEvents
        XCTAssertTrue(driver.start())
        XCTAssertEqual(driver.creations, 2)
    }

    private func health(
        authorized: Bool = true, valid: Bool = true, enabled: Bool = true,
        events: CGEventMask? = EventTapHealth.requiredEvents
    ) -> EventTapHealth {
        EventTapHealth(authorized: authorized, valid: valid, enabled: enabled, events: events)
    }

    private func tap(id: UInt32, process: pid_t = 101, events: CGEventMask) -> CGEventTapInformation {
        var info = CGEventTapInformation()
        info.eventTapID = id
        info.tappingProcess = process
        info.tapPoint = .cgSessionEventTap
        info.options = .listenOnly
        info.eventsOfInterest = events
        info.enabled = true
        return info
    }
}

private final class FakeTap {
    var authorized: Bool
    var events: CGEventMask?
    var createdEvents = EventTapHealth.requiredEvents
    var creations = 0

    init(authorized: Bool, events: CGEventMask?) {
        self.authorized = authorized
        self.events = events
    }

    func start() -> Bool {
        EventTapRecovery.restore(
            isAvailable: { EventTapHealth(authorized: self.authorized, valid: self.events != nil, enabled: true, events: self.events).isAvailable },
            authorized: { self.authorized },
            remove: { self.events = nil },
            create: {
                self.creations += 1
                self.events = self.createdEvents
                return true
            }
        )
    }
}
