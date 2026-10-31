import Foundation
import Testing
@testable import PennelMobileKit

struct SessionTests {
    @Test func session_clock_expires_and_extends() {
        let start = Date(timeIntervalSince1970: 1_000)
        var clock = SessionClock(duration: .fiveMinutes, now: start)
        #expect(clock.duration == 300)
        #expect(!clock.isExpired(now: start.addingTimeInterval(299)))
        #expect(clock.isExpired(now: start.addingTimeInterval(300)))

        clock.extend(now: start.addingTimeInterval(200))
        #expect(!clock.isExpired(now: start.addingTimeInterval(450)))
        #expect(clock.isExpired(now: start.addingTimeInterval(500)))
        #expect(clock.remaining(now: start.addingTimeInterval(400)) == 100)
        #expect(SessionClock.Duration.fifteenMinutes.rawValue == 900)
        #expect(SessionClock.Duration.oneHour.rawValue == 3600)
    }

    @Test func ready_samples_are_discarded() {
        let buffer = DictationBuffer()
        buffer.append([0.1, 0.2], phase: .ready)
        buffer.append([0.3], phase: .off)
        buffer.append([0.4], phase: .processing)
        #expect(buffer.take().isEmpty)
    }

    @Test func listening_samples_are_kept_and_taken_once() {
        let buffer = DictationBuffer()
        buffer.append([0.1], phase: .ready)
        buffer.append([0.2, 0.3], phase: .listening)
        buffer.append([0.4], phase: .ready)
        buffer.append([0.5], phase: .listening)
        #expect(buffer.take() == [0.2, 0.3, 0.5])
        #expect(buffer.take().isEmpty)
    }
}
