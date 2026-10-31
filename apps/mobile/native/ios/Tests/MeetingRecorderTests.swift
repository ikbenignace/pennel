import AVFoundation
import Foundation
import PennelMobileKit
import Testing
@testable internal import PennelMobile

private final class RecordingSink: MeetingAudioSink, @unchecked Sendable {
    private let lock = NSLock()
    private var blocks: [(startMs: UInt64, count: Int, silent: Bool)] = []
    var pushed: [(startMs: UInt64, count: Int, silent: Bool)] { lock.withLock { blocks } }

    func push(startMs: UInt64, samples: [Float]) throws {
        lock.withLock { blocks.append((startMs, samples.count, samples.allSatisfy { $0 == 0 })) }
    }
}

@MainActor
struct MeetingRecorderTests {
    @Test func meeting_audio_goes_to_others_track_with_timestamps() async throws {
        let h = try MeetingHarness()
        try await h.runtime.start(title: "Overleg")
        let id = try #require(h.runtime.activeMeetingId)
        #expect(h.capture.running)
        #expect(h.audio.active)

        h.speak(seconds: 20)
        await h.runtime.stop()

        let transcript = h.transcript(id)
        #expect(!transcript.isEmpty)
        #expect(transcript.allSatisfy { $0.speaker == .others && !$0.failed })
        #expect(transcript.first?.startMs == 0)
        #expect(transcript.first?.text == "hallo wereld")
        #expect(h.transcriber.calls.reduce(0, +) == 20 * 16_000)
        let archive = URL(fileURLWithPath: h.engine.meetingDir(id: id)).appendingPathComponent("audio/room.m4a")
        #expect(FileManager.default.fileExists(atPath: archive.path))
        #expect(!FileManager.default.fileExists(atPath: archive.deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("anderen.m4a").path))
        #expect(h.engine.meetingAudioTracks(id: id).first?.source == .roomMicrophone)
        #expect(h.engine.meetingArchiveURL(id: id, speaker: .others) == archive.resolvingSymlinksInPath())
        let file = try AVAudioFile(forReading: archive)
        #expect(abs(Double(file.length) / file.fileFormat.sampleRate - 20) < 0.5)
        #expect(!h.capture.running)
        #expect(!h.audio.active)
        #expect(h.finished.first?.isComplete == true)
    }

    @Test func interruption_leaves_gap_and_resumes() async throws {
        let h = try MeetingHarness()
        try await h.runtime.start(title: "Overleg")
        let id = try #require(h.runtime.activeMeetingId)
        h.speak(seconds: 16)

        h.interruption(.began)
        #expect(h.runtime.phase == .interrupted)
        #expect(!h.capture.running)
        #expect(!h.audio.active)
        h.silence(seconds: 30)
        h.interruption(.ended, shouldResume: true)
        #expect(h.runtime.phase == .recording)
        #expect(h.capture.running)
        #expect(h.capture.starts == 2)
        h.speak(seconds: 16)
        await h.runtime.stop()

        let transcript = h.transcript(id)
        #expect(transcript.count == 2)
        #expect(transcript.first?.startMs == 0)
        #expect(h.transcriber.calls == [16 * 16_000, 16 * 16_000])
        #expect(transcript.last?.startMs == 46_000)
        let finish = try #require(h.finished.first)
        #expect(finish.gaps == [MeetingGap(startMs: 16_000, endMs: 46_000)])
        #expect(finish.stoppedEarly == nil)
        #expect(!finish.isComplete)
        #expect(h.interruptions.record(for: finish.id)?.gaps == finish.gaps)
        let archive = URL(fileURLWithPath: h.engine.meetingDir(id: finish.id)).appendingPathComponent("audio/room.m4a")
        let file = try AVAudioFile(forReading: archive)
        #expect(abs(Double(file.length) / file.fileFormat.sampleRate - 62) < 0.5)
        let metrics = try recordingMetadata(h.engine, id: id)
        #expect(metrics["complete"] as? Bool == false)
        let events = try captureEvents(h.engine, id: id)
        #expect(events.contains { $0["kind"] as? String == "interruption" })
    }

    @Test func dictation_keeps_meeting_audio_silent_and_marks_interval() async throws {
        let h = try MeetingHarness()
        try await h.runtime.start(title: "Overleg")
        let id = try #require(h.runtime.activeMeetingId)
        let hook: DictationMeetingHook = h.runtime
        #expect(hook.isActive)
        h.speak(seconds: 16)

        let start = hook.elapsedMs
        hook.setDictating(true)
        h.speak(seconds: 16)
        hook.setDictating(false)
        hook.markDictation(startMs: start, endMs: hook.elapsedMs)
        await h.runtime.stop()

        #expect(start == 16_000)
        #expect(h.transcript(id).count == 1)
        #expect(h.transcriber.calls.count == 1)
    }

    @Test func recorder_pushes_whole_seconds_and_jumps_gaps() throws {
        let clock = ManualClock()
        let sink = RecordingSink()
        let recorder = MeetingRecorder(sink: sink, archive: nil, clock: { clock.now })
        recorder.begin()
        for _ in 0..<25 {
            clock.advance(0.1)
            recorder.feed(meetingTone(seconds: 0.1))
        }
        recorder.pause()
        clock.advance(10)
        recorder.resume()
        for _ in 0..<10 {
            clock.advance(0.1)
            recorder.feed(meetingTone(seconds: 0.1))
        }
        #expect(recorder.finish() == nil)

        #expect(sink.pushed.map(\.startMs) == [0, 1_000, 2_000, 12_500])
        #expect(sink.pushed.map(\.count) == [16_000, 16_000, 8_000, 16_000])
        #expect(recorder.gaps == [MeetingGap(startMs: 2_500, endMs: 12_500)])
        #expect(recorder.metrics.outputSamples == 13_500 * 16)
        #expect(recorder.metrics.insertedSilenceSamples == 10_000 * 16)
    }

    @Test func legacy_archives_remain_readable_without_recording_metadata() throws {
        let h = try MeetingHarness()
        let session = try h.engine.startMeeting(title: "Legacy", keys: ApiKeys(byProvider: [:]))
        try session.stop()
        let id = session.id()
        let root = URL(fileURLWithPath: h.engine.meetingDir(id: id))
        let legacy = root.appendingPathComponent("anderen.m4a")
        try Data([1, 2, 3]).write(to: legacy)
        #expect(h.engine.meetingArchiveURL(id: id, speaker: .others) == legacy.resolvingSymlinksInPath())
        #expect(h.engine.meetingArchiveURL(id: id, speaker: .me) == nil)
        #expect(!FileManager.default.fileExists(atPath: root.appendingPathComponent(".pennel/recording.json").path))
    }

    @Test func completed_room_recording_keeps_local_provenance() async throws {
        let h = try MeetingHarness()
        try await h.runtime.start(title: "Overleg")
        let id = try #require(h.runtime.activeMeetingId)
        h.speak(seconds: 3)
        await h.runtime.stop()

        let metadata = try recordingMetadata(h.engine, id: id)
        #expect(metadata["complete"] as? Bool == true)
        #expect(metadata["duration_ms"] as? UInt64 == 3_000)
        let recording = try #require(try h.engine.meetingRecordingMetadata(id: id))
        #expect(recording.tracks.first?.sampleRate == 16_000)
        #expect(recording.tracks.first?.formatEpochs.first?.inputSampleRate == 24_000)
        let text = String(decoding: try JSONSerialization.data(withJSONObject: metadata), as: UTF8.self)
        #expect(text.contains("room_microphone"))
        #expect(!text.contains(h.dataDir.path))
        #expect(!text.contains("hallo wereld"))
        #expect(try captureEvents(h.engine, id: id).contains { $0["kind"] as? String == "stopped" })
    }
}

private func recordingMetadata(_ engine: PennelEngine, id: String) throws -> [String: Any] {
    let url = URL(fileURLWithPath: engine.meetingDir(id: id)).appendingPathComponent(".pennel/recording.json")
    let value = try JSONSerialization.jsonObject(with: Data(contentsOf: url))
    return try #require(value as? [String: Any])
}

private func captureEvents(_ engine: PennelEngine, id: String) throws -> [[String: Any]] {
    let url = URL(fileURLWithPath: engine.meetingDir(id: id)).appendingPathComponent(".pennel/capture-events.jsonl")
    return try String(contentsOf: url, encoding: .utf8).split(separator: "\n").map {
        let value = try JSONSerialization.jsonObject(with: Data($0.utf8))
        return try #require(value as? [String: Any])
    }
}
