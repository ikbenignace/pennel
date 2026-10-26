import AVFoundation
import Foundation
import PennelMobileKit
import Testing
@testable import Pennel

private final class CountingCapture: AudioCapturing, @unchecked Sendable {
    private let lock = NSLock()
    private var onSamples: (@Sendable ([Float]) -> Void)?
    private var alive = false
    private(set) var starts = 0
    private(set) var stops = 0

    var isRunning: Bool { lock.withLock { alive } }

    func die() {
        lock.withLock { alive = false }
    }

    func start(onSamples: @escaping @Sendable ([Float]) -> Void) throws {
        lock.withLock {
            starts += 1
            alive = true
            self.onSamples = onSamples
        }
    }

    func stop() {
        lock.withLock {
            stops += 1
            alive = false
            onSamples = nil
        }
    }

    func feed(_ samples: [Float]) {
        lock.withLock { onSamples }?(samples)
    }
}

private final class Received: @unchecked Sendable {
    private let lock = NSLock()
    private var total = 0
    var count: Int { lock.withLock { total } }
    func add(_ samples: [Float]) { lock.withLock { total += samples.count } }
}

@MainActor
struct MeetingRuntimeTests {
    /// iOS only grants continued processing to a request made from the user's action in the foreground, so the
    /// task is requested at Stop opname and keeps running if the user then leaves the app.
    @Test func stop_during_background_schedules_finish_task() async throws {
        let h = try MeetingHarness()
        try await h.runtime.start(title: "Overleg")
        h.speak(seconds: 5)
        await h.runtime.stop()

        #expect(h.scheduler.begun == 1)
        #expect(h.scheduler.task.ended == [true])
    }

    @Test func failed_capture_is_not_reported_complete() async throws {
        let h = try MeetingHarness()
        try await h.runtime.start(title: "Overleg")
        let id = try #require(h.runtime.activeMeetingId)
        h.speak(seconds: 16)

        h.interruption(.began)
        h.silence(seconds: 20)
        h.capture.failNextStarts()
        h.interruption(.ended, shouldResume: true)
        await h.runtime.stop()

        #expect(h.finished.count == 1)
        let finish = try #require(h.finished.first)
        #expect(finish.id == id)
        #expect(!finish.isComplete)
        #expect(finish.stoppedEarly == MeetingRuntime.Message.couldNotResume)
        #expect(finish.gaps == [MeetingGap(startMs: 16_000, endMs: nil)])
        #expect(h.interruptions.record(for: id)?.stoppedEarly == MeetingRuntime.Message.couldNotResume)
        #expect(h.runtime.phase == .idle)
        #expect(h.runtime.activeMeetingId == nil)
        #expect(!h.capture.running)
        #expect(!h.audio.active)
        #expect(h.activity.ended == 1)
        #expect(h.transcript(id).count == 1)
        #expect(h.engine.meetingInfo(id: id)?.hasSummary == false)
    }

    @Test func active_meeting_id_exposed_while_recording() async throws {
        let h = try MeetingHarness()
        #expect(h.runtime.activeMeetingId == nil)
        try await h.runtime.start(title: "Overleg")
        let id = try #require(h.runtime.activeMeetingId)
        #expect(h.engine.meetings().map(\.id) == [id])
        #expect(h.runtime.isActive)
        #expect(h.activity.started.count == 1)
        await #expect(throws: (any Error).self) { try await h.runtime.start(title: "Tweede") }

        h.speak(seconds: 3)
        await h.runtime.stop()

        #expect(h.runtime.activeMeetingId == nil)
        #expect(!h.runtime.isActive)
        #expect(h.finished.map(\.id) == [id])
        #expect(h.finished.first?.isComplete == true)
        #expect(h.interruptions.record(for: id) == nil)
    }

    @Test func live_notes_survive_a_reopened_view_and_stop() async throws {
        let h = try MeetingHarness()
        try await h.runtime.start(title: "Overleg")
        let id = try #require(h.runtime.activeMeetingId)
        h.runtime.setNotes("Budget beslist")

        // A reopened live view starts from the runtime's notes and sends them again on Stop opname.
        let reopened = h.runtime.notes
        #expect(reopened == "Budget beslist")
        h.runtime.setNotes(reopened)
        h.speak(seconds: 2)
        await h.runtime.stop()

        #expect(h.engine.meetingNotes(id: id) == "Budget beslist")
        #expect(h.runtime.notes.isEmpty)
    }

    @Test func stalled_capture_restarts_and_leaves_a_gap() async throws {
        let h = try MeetingHarness()
        try await h.runtime.start(title: "Overleg")
        h.speak(seconds: 5)
        h.silence(seconds: 3)
        h.runtime.tick()
        #expect(h.capture.starts == 2)
        #expect(h.runtime.warning == MeetingRuntime.Message.microphoneLost)
        #expect(h.runtime.phase == .recording)
        h.speak(seconds: 5)
        h.runtime.tick()
        #expect(h.runtime.warning == nil)
        await h.runtime.stop()

        let finish = try #require(h.finished.first)
        #expect(finish.gaps == [MeetingGap(startMs: 5_000, endMs: 8_000)])
        #expect(!finish.isComplete)
    }

    @Test func stall_until_stop_is_not_reported_complete() async throws {
        let h = try MeetingHarness()
        try await h.runtime.start(title: "Overleg")
        h.speak(seconds: 5)
        h.silence(seconds: 1.5)
        await h.runtime.stop()

        let finish = try #require(h.finished.first)
        #expect(finish.gaps == [MeetingGap(startMs: 5_000, endMs: nil)])
        #expect(!finish.isComplete)
        #expect(h.interruptions.record(for: finish.id) != nil)
    }

    @Test func failed_restart_after_a_stall_finishes_once_as_stopped_early() async throws {
        let h = try MeetingHarness()
        try await h.runtime.start(title: "Overleg")
        h.speak(seconds: 5)
        h.silence(seconds: 3)
        h.capture.failNextStarts()
        h.runtime.tick()
        h.runtime.tick()
        await h.runtime.stop()

        #expect(h.finished.count == 1)
        #expect(h.finished.first?.stoppedEarly == MeetingRuntime.Message.couldNotResume)
        #expect(h.finished.first?.isComplete == false)
        #expect(h.runtime.phase == .idle)
    }

    @Test func shared_microphone_restarts_a_dead_engine() throws {
        let capture = CountingCapture()
        let center = NotificationCenter()
        let microphone = SharedMicrophone(capture: capture, notifications: center)
        let meeting = microphone.client()
        let received = Received()
        try meeting.start { received.add($0) }

        center.post(name: .AVAudioEngineConfigurationChange, object: nil)
        #expect(capture.starts == 1)

        capture.die()
        #expect(!meeting.isRunning)
        center.post(name: AVAudioSession.routeChangeNotification, object: nil)
        #expect(capture.starts == 2)
        #expect(meeting.isRunning)

        // A media-services reset kills the engine; the next client must not trust the stale running flag.
        capture.die()
        let session = microphone.client()
        try session.start { _ in }
        #expect(capture.starts == 3)
        capture.feed([0.1])
        #expect(received.count == 1)
    }

    @Test func library_groups_follow_the_desktop() throws {
        func meeting(_ iso: String) -> MeetingInfo {
            MeetingInfo(id: iso, title: "T", startIso: iso, durationMin: 1, hasSummary: false)
        }
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = try #require(TimeZone(identifier: "Europe/Brussels"))
        let now = try #require(MeetingDates.date("2026-10-26T15:00:00+01:00"))
        let groups = MeetingDates.groups(
            [meeting("2026-10-26T09:00:00+01:00"), meeting("2026-10-25T18:00:00+01:00"), meeting("2026-10-10T10:00:00+02:00")],
            now: now, calendar: calendar)
        #expect(groups.map(\.title) == [Copy.Meetings.today, Copy.Meetings.thisWeek, Copy.Meetings.earlier])
        #expect(groups.map(\.title) == ["Vandaag", "Deze week", "Eerder"])
    }

    @Test func shared_microphone_runs_one_capture_for_meeting_and_session() throws {
        let capture = CountingCapture()
        let microphone = SharedMicrophone(capture: capture)
        let meeting = microphone.client()
        let session = microphone.client()
        let meetingSamples = Received()
        let sessionSamples = Received()

        try meeting.start { meetingSamples.add($0) }
        try session.start { sessionSamples.add($0) }
        capture.feed([0.1, 0.2])
        #expect(capture.starts == 1)
        #expect(meetingSamples.count == 2)
        #expect(sessionSamples.count == 2)

        session.stop()
        capture.feed([0.3])
        #expect(capture.stops == 0)
        #expect(meetingSamples.count == 3)
        #expect(sessionSamples.count == 2)

        meeting.stop()
        #expect(capture.stops == 1)
    }
}
