import AVFoundation
import Foundation
import Testing
@testable import Pennel

private final class CountingCapture: AudioCapturing, @unchecked Sendable {
    private let lock = NSLock()
    private var onSamples: (@Sendable ([Float]) -> Void)?
    private(set) var starts = 0
    private(set) var stops = 0

    func start(onSamples: @escaping @Sendable ([Float]) -> Void) throws {
        lock.withLock {
            starts += 1
            self.onSamples = onSamples
        }
    }

    func stop() {
        lock.withLock {
            stops += 1
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
    @Test func stop_during_background_schedules_finish_task() async throws {
        let h = try MeetingHarness()
        try h.runtime.start(title: "Overleg")
        h.speak(seconds: 5)
        h.appActive = false
        await h.runtime.stop()

        #expect(h.scheduler.begun == 1)
        #expect(h.scheduler.task.ended == [true])

        let foreground = try MeetingHarness()
        try foreground.runtime.start(title: "Overleg")
        foreground.speak(seconds: 5)
        await foreground.runtime.stop()
        #expect(foreground.scheduler.begun == 0)
    }

    @Test func failed_capture_is_not_reported_complete() async throws {
        let h = try MeetingHarness()
        try h.runtime.start(title: "Overleg")
        let id = try #require(h.runtime.activeMeetingId)
        h.speak(seconds: 16)

        h.interruption(.began)
        h.silence(seconds: 20)
        h.capture.failNextStarts()
        h.interruption(.ended, shouldResume: true)
        await h.runtime.finishing?.value

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
        try h.runtime.start(title: "Overleg")
        let id = try #require(h.runtime.activeMeetingId)
        #expect(h.engine.meetings().map(\.id) == [id])
        #expect(h.runtime.isActive)
        #expect(h.activity.started.count == 1)
        #expect(throws: (any Error).self) { try h.runtime.start(title: "Tweede") }

        h.speak(seconds: 3)
        await h.runtime.stop()

        #expect(h.runtime.activeMeetingId == nil)
        #expect(!h.runtime.isActive)
        #expect(h.finished.map(\.id) == [id])
        #expect(h.finished.first?.isComplete == true)
        #expect(h.interruptions.record(for: id) == nil)
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
