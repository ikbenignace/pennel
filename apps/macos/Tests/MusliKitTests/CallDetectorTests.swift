import MusliKit
import XCTest

final class CallDetectorTests: XCTestCase {
    private let teams = MicUser(bundleId: "com.microsoft.teams2", isCall: true, isPlaying: true)
    private let zoom = MicUser(bundleId: "us.zoom.xos", isCall: true, isPlaying: true)

    private func detector() -> CallDetector {
        CallDetector(ownBundleId: "dev.musli.Musli")
    }

    func testShortUseDoesNotStart() {
        var d = detector()
        XCTAssertEqual(d.update(users: [teams], at: 0), [])
        XCTAssertEqual(d.update(users: [teams], at: 3), [])
        XCTAssertEqual(d.update(users: [], at: 4), [])
        XCTAssertEqual(d.update(users: [teams], at: 6), [])
        XCTAssertNil(d.activeCall)
    }

    func testSustainedCallStartsOnce() {
        var d = detector()
        _ = d.update(users: [teams], at: 0)
        XCTAssertEqual(d.update(users: [teams], at: 5), [.started("com.microsoft.teams2")])
        XCTAssertEqual(d.update(users: [teams], at: 10), [])
        XCTAssertEqual(d.activeCall, "com.microsoft.teams2")
    }

    func testBriefMuteDoesNotEnd() {
        var d = detector()
        _ = d.update(users: [teams], at: 0)
        _ = d.update(users: [teams], at: 5)
        XCTAssertEqual(d.update(users: [], at: 10), [])
        XCTAssertEqual(d.update(users: [], at: 30), [])
        XCTAssertEqual(d.update(users: [teams], at: 35), [])
        XCTAssertEqual(d.update(users: [], at: 60), [])
        XCTAssertEqual(d.activeCall, "com.microsoft.teams2")
    }

    func testEndsAfterThirtySeconds() {
        var d = detector()
        _ = d.update(users: [teams], at: 0)
        _ = d.update(users: [teams], at: 5)
        _ = d.update(users: [], at: 10)
        XCTAssertEqual(d.update(users: [], at: 40), [.ended("com.microsoft.teams2")])
        XCTAssertNil(d.activeCall)
    }

    func testOwnAppIsIgnored() {
        var d = detector()
        let own = MicUser(bundleId: "dev.musli.Musli", isCall: true, isPlaying: true)
        _ = d.update(users: [own], at: 0)
        XCTAssertEqual(d.update(users: [own], at: 10), [])
    }

    func testNonCallAppsAreIgnored() {
        var d = detector()
        let memo = MicUser(bundleId: "com.apple.VoiceMemos", isCall: false, isPlaying: false)
        _ = d.update(users: [memo], at: 0)
        XCTAssertEqual(d.update(users: [memo], at: 10), [])
    }

    func testSecondAppWhileActiveIsIgnored() {
        var d = detector()
        _ = d.update(users: [teams], at: 0)
        _ = d.update(users: [teams], at: 5)
        XCTAssertEqual(d.update(users: [teams, zoom], at: 6), [])
        XCTAssertEqual(d.update(users: [teams, zoom], at: 20), [])
        XCTAssertEqual(d.activeCall, "com.microsoft.teams2")
    }

    func testVoiceMessageWithoutPlaybackDoesNotStart() {
        var d = detector()
        let note = MicUser(bundleId: "net.whatsapp.WhatsApp", isCall: true, isPlaying: false)
        _ = d.update(users: [note], at: 0)
        XCTAssertEqual(d.update(users: [note], at: 10), [])
        XCTAssertNil(d.activeCall)
    }

    func testBrowserCallSurvivesTabSwitch() {
        var d = detector()
        let meet = MicUser(bundleId: "com.google.Chrome", isCall: true, isPlaying: true)
        let otherTab = MicUser(bundleId: "com.google.Chrome", isCall: false, isPlaying: true)
        _ = d.update(users: [meet], at: 0)
        XCTAssertEqual(d.update(users: [meet], at: 5), [.started("com.google.Chrome")])
        XCTAssertEqual(d.update(users: [otherTab], at: 20), [])
        XCTAssertEqual(d.update(users: [otherTab], at: 60), [])
        XCTAssertEqual(d.activeCall, "com.google.Chrome")
    }
}
