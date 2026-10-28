import AVFoundation
import Foundation
import Testing
import PennelMobileKit
@testable internal import PennelMobile

private final class UploadProgress: RecognitionUploadProgress, @unchecked Sendable {
    func sent(sentBytes: UInt64, totalBytes: UInt64) {}
}

private final class FixtureCapture: AudioCapturing, @unchecked Sendable {
    private let lock = NSLock()
    private var count = 0
    private var running = false
    var starts: Int { lock.withLock { count } }
    var isRunning: Bool { lock.withLock { running } }
    func start(onSamples: @escaping @Sendable ([Float]) -> Void) throws { lock.withLock { count += 1; running = true } }
    func stop() { lock.withLock { running = false } }
}

private struct FixtureAudioSession: AudioSessionControlling {
    func activate() throws {}
    func deactivate() {}
}

private struct QuietDictationRunner: DictationRunning {
    func run(samples: [Float]) async -> Result<DictationResult, PennelError> { .failure(.NoSpeech) }
}

@MainActor
private final class FixtureSessionActivity: SessionActivityControlling {
    func start(_ state: SessionActivityState) {}
    func update(_ state: SessionActivityState) {}
    func end(message: String?) {}
}

@MainActor
private final class PreflightGate {
    private var pending: CheckedContinuation<Bool, Never>?
    private var waiting: [CheckedContinuation<Void, Never>] = []
    private(set) var entered = false
    func request() async -> Bool {
        entered = true
        waiting.forEach { $0.resume() }
        waiting = []
        return await withCheckedContinuation { pending = $0 }
    }
    func waitUntilEntered() async {
        if entered { return }
        await withCheckedContinuation { waiting.append($0) }
    }
    func resolve() { pending?.resume(returning: true); pending = nil }
}

@Suite("Expo native runtime", .serialized)
struct PennelRuntimeTests {
    @MainActor @Test func nativeFailuresUseEnglishDiagnosticsAndStableCodes() throws {
        #expect(ErrorText.message(PennelError.NoSpeech) == "No speech recognized. Try again.")
        #expect(ErrorText.failureCode(PennelError.NoSpeech) == "no_speech")
        #expect(ErrorText.failureCode(PennelError.Http(status: 401, message: "Private provider details")) == "provider_auth")
        #expect(ErrorText.failureCode(PennelError.Http(status: 403, message: "Private provider details")) == "provider_auth")
        #expect(ErrorText.failureCode(PennelError.NotSignedIn) == "not_signed_in")
        #expect(ErrorText.code(PennelError.Cloud(code: "private provider code", message: "private provider details")) == "cloud")
        #expect(NativeLanguageModelError.invalidRequest.errorDescription == "Invalid request for the built-in Apple text model.")
        #expect(NativeLanguageModelAvailability.unavailable(.notReady).reason?.contains("Wait until iOS") == true)
        #expect(MeetingRuntime.Message.code(for: "De opname is gestopt omdat iOS Pennel heeft afgesloten.") == "recording_terminated")
        #expect(MeetingRuntime.Message.code(for: MeetingRuntime.Message.terminated) == "recording_terminated")
        #expect(CloudAccountModel.isMassDelete(CloudAccountModel.massDelete))
        #expect(CloudAccountModel.isMassDelete("Veel meetings zijn verdwenen uit je map. Sync heeft niets verwijderd; bevestig in Instellingen → Account als dat de bedoeling was."))
        let legacy = Data("{\"gaps\":[],\"stoppedEarly\":\"De opname is gestopt omdat iOS Pennel heeft afgesloten.\"}".utf8)
        let saved = try JSONDecoder().decode(MeetingInterruptionLog.Record.self, from: legacy)
        #expect(saved.stoppedEarlyCode == nil)
        #expect(saved.stoppedEarly == "De opname is gestopt omdat iOS Pennel heeft afgesloten.")
    }

    @Test func roomAndLegacyArchivesUseTheSharedResolver() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let engine = try PennelEngine.newPreview(dataDir: directory.path)
        let session = try engine.startMeeting(title: "Room fixture", keys: ApiKeys(byProvider: [:]))
        let id = session.id()
        let tracks = try engine.prepareMeetingRecording(id: id, environment: RecordingEnvironment(
            appVersion: "test", build: "test", platform: "ios", osVersion: "test"
        ), roomRecording: true)
        let track = try #require(tracks.first)
        #expect(track.source == .roomMicrophone)
        #expect(track.speaker == .others)
        #expect(track.relativePath == "audio/room.m4a")
        let root = URL(fileURLWithPath: engine.meetingDir(id: id))
        let room = root.appendingPathComponent(track.relativePath)
        try Data([1, 2, 3]).write(to: room)
        #expect(engine.meetingArchiveURL(id: id, speaker: .others) == room.resolvingSymlinksInPath())
        try session.stop()
        try engine.finishMeetingRecording(id: id, durationMs: 1_234, complete: true)
        #expect(try engine.meetingRecordingMetadata(id: id)?.durationMs == 1_234)

        let legacy = try engine.startMeeting(title: "Legacy fixture", keys: ApiKeys(byProvider: [:]))
        try legacy.stop()
        let legacyID = legacy.id()
        let legacyURL = URL(fileURLWithPath: engine.meetingDir(id: legacyID)).appendingPathComponent("anderen.m4a")
        try Data([1, 2, 3]).write(to: legacyURL)
        #expect(engine.meetingArchiveURL(id: legacyID, speaker: .others) == legacyURL.resolvingSymlinksInPath())
        #expect(try engine.meetingRecordingMetadata(id: legacyID) == nil)
    }

    @MainActor @Test func staleRetryCannotSaveAnotherRecording() async throws {
        _ = try PennelNativeRuntime.shared.bootstrap()
        do {
            _ = try await PennelNativeRuntime.shared.invoke(method: "meetings.retrySave", payload: "{\"id\":\"not-a-retained-recording\"}")
            Issue.record("A stale retry must reject instead of reporting a save.")
        } catch let error as PennelBridgeError { #expect(error.code == "meeting_changed") }
    }

    @Test func stoppedModuleObservationCannotInstallALateListener() throws {
        let observation = ModuleObservation()
        let first = try #require(observation.begin())
        #expect(observation.remove(permanently: false) == nil)
        #expect(!observation.install(UUID(), generation: first.generation))
        let second = try #require(observation.begin())
        let id = UUID()
        #expect(observation.install(id, generation: second.generation))
        let third = try #require(observation.begin())
        #expect(third.previous == id)
        #expect(!observation.install(UUID(), generation: second.generation))
        #expect(observation.remove(permanently: true) == nil)
        #expect(!observation.install(UUID(), generation: third.generation))
        #expect(observation.begin() == nil)
    }

    @MainActor @Test func cancelledPermissionPreflightCannotStartTheMicrophone() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let engine = try PennelEngine.newPreview(dataDir: directory.path)
        let capture = FixtureCapture()
        let permission = PreflightGate()
        let recorder = NativeDictationRecorder(engine: engine, capture: capture, audio: FixtureAudioSession(), requestPermission: { await permission.request() }, reserveHistory: {}, observeSystem: false)
        let first = Task { try await recorder.start() }
        await permission.waitUntilEntered()
        try await recorder.start()
        recorder.cancel()
        permission.resolve()
        do { try await first.value; Issue.record("Cancelled preflight must reject.") }
        catch PennelError.Cancelled {}
        #expect(capture.starts == 0)
        #expect(recorder.phase == .idle)
    }

    @MainActor @Test func cancelledCapacityPreflightCannotStartTheMicrophone() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let engine = try PennelEngine.newPreview(dataDir: directory.path)
        let capture = FixtureCapture()
        let capacity = PreflightGate()
        let recorder = NativeDictationRecorder(engine: engine, capture: capture, audio: FixtureAudioSession(), requestPermission: { true }, reserveHistory: { _ = await capacity.request() }, observeSystem: false)
        let first = Task { try await recorder.start() }
        await capacity.waitUntilEntered()
        recorder.cancel()
        capacity.resolve()
        do { try await first.value; Issue.record("Cancelled capacity preflight must reject.") }
        catch PennelError.Cancelled {}
        #expect(capture.starts == 0)
        #expect(recorder.phase == .idle)
    }

    @MainActor @Test func concurrentStartsCreateOnlyOneCapture() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let engine = try PennelEngine.newPreview(dataDir: directory.path)
        let capture = FixtureCapture()
        let permission = PreflightGate()
        let recorder = NativeDictationRecorder(engine: engine, capture: capture, audio: FixtureAudioSession(), requestPermission: { await permission.request() }, reserveHistory: {}, observeSystem: false)
        let first = Task { try await recorder.start() }
        await permission.waitUntilEntered()
        try await recorder.start()
        permission.resolve()
        try await first.value
        #expect(capture.starts == 1)
        recorder.cancel()
    }

    @MainActor @Test func stoppedKeyboardSessionCannotStartAfterPermissionReturns() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let capture = FixtureCapture()
        let permission = PreflightGate()
        let session = DictationSession(capture: capture, audioSession: FixtureAudioSession(), runner: { QuietDictationRunner() }, store: HandoffStore(directory: directory), activity: FixtureSessionActivity(), observeCommands: false, requestPermission: { await permission.request() })
        let starting = Task { await session.start() }
        await permission.waitUntilEntered()
        session.stop()
        permission.resolve()
        #expect(await starting.value == false)
        #expect(capture.starts == 0)
        #expect(session.phase == .off)
    }

    @MainActor @Test func cancelledPreviewCannotReplaceTheNextDictation() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let engine = try PennelEngine.newPreview(dataDir: directory.path)
        let capture = FixtureCapture()
        let recorder = NativeDictationRecorder(engine: engine, capture: capture, audio: FixtureAudioSession(), requestPermission: { true }, reserveHistory: {}, observeSystem: false)
        try await recorder.start()
        let old = try #require(recorder.previewObserver)
        recorder.cancel()
        try await recorder.start()
        old.receive(DictationPreviewUpdate(sessionId: "old", revision: 1, text: "stale fixture"))
        #expect(recorder.phase == .listening)
        #expect(recorder.text.isEmpty)
        #expect(capture.starts == 2)
        recorder.cancel()
    }

    @MainActor @Test func replacedLoginGenerationCannotClearTheNewFlow() {
        let generation = NativeLoginGeneration()
        let old = generation.begin()
        generation.cancel()
        let replacement = generation.begin()
        #expect(!generation.isCurrent(old))
        #expect(generation.isCurrent(replacement))
    }

    @MainActor @Test func obsoleteQueuedLoginStartsAreDiscarded() async throws {
        let queue = NativeLoginStartQueue()
        let generation = NativeLoginGeneration()
        let gate = PreflightGate()
        let firstEpoch = generation.begin()
        let first = Task {
            try await queue.run(generation: firstEpoch, owner: generation) {
                _ = await gate.request()
                return CloudLoginStart(userCode: "fixture", verificationUri: "https://example.invalid", deviceCode: "native-only", intervalSecs: 5)
            }
        }
        await gate.waitUntilEntered()
        let obsoleteEpoch = generation.begin()
        let obsolete = Task {
            try await queue.run(generation: obsoleteEpoch, owner: generation) {
                Issue.record("An obsolete queued login must not reach the native request.")
                return CloudLoginStart(userCode: "obsolete", verificationUri: "https://example.invalid", deviceCode: "native-only", intervalSecs: 5)
            }
        }
        generation.cancel()
        let latestEpoch = generation.begin()
        let latest = Task {
            try await queue.run(generation: latestEpoch, owner: generation) {
                CloudLoginStart(userCode: "latest", verificationUri: "https://example.invalid", deviceCode: "native-only", intervalSecs: 5)
            }
        }
        gate.resolve()
        do { _ = try await first.value; Issue.record("Old in-flight start must reject.") } catch PennelError.Cancelled {}
        do { _ = try await obsolete.value; Issue.record("Old queued start must reject.") } catch PennelError.Cancelled {}
        #expect(try await latest.value.userCode == "latest")
    }

    @MainActor @Test func intentsBeforeJavaScriptCoalesceToTheLatestRoute() throws {
        _ = try PennelNativeRuntime.shared.bootstrap()
        let router = AppRouter.shared
        router.requestLiveMeeting()
        router.requestDictation()
        let first = try BridgeJSON.object(PennelNativeRuntime.shared.getSnapshot())["navigation"] as? [String: Any]
        #expect(first?["route"] as? String == "dictation")
        router.requestDictation()
        router.requestLiveMeeting()
        let second = try BridgeJSON.object(PennelNativeRuntime.shared.getSnapshot())["navigation"] as? [String: Any]
        #expect(second?["route"] as? String == "meeting")
    }
    @Test func boundedArchiveReadsSeekAndEndWithoutWholeRecordingBuffers() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let archive = directory.appendingPathComponent("fixture.caf")
        let format = try #require(AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 16_000, channels: 1, interleaved: false))
        var writer: AVAudioFile? = try AVAudioFile(forWriting: archive, settings: format.settings)
        let audio = try #require(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 64_000))
        audio.frameLength = 64_000
        for index in 0..<64_000 { audio.floatChannelData![0][index] = Float(index % 100) / 100 }
        try writer?.write(from: audio)
        try writer?.write(from: audio)
        writer = nil

        let reader = NativeRecognitionReader(archive: archive)
        let info = try reader.info()
        #expect(info.totalFrames == 128_000)
        #expect(info.audioDigest.count == 64)
        let middle = try reader.readFrames(offset: 64_010, maxFrames: 64_000)
        #expect(middle.samples.count == 63_990)
        #expect(abs(middle.samples[0] - 0.1) < 0.0001)
        #expect(middle.eof)
        let end = try reader.readFrames(offset: 128_000, maxFrames: 64_000)
        #expect(end.samples.isEmpty)
        #expect(end.eof)
        #expect(throws: Error.self) { try reader.readFrames(offset: 0, maxFrames: 64_001) }
        #expect(throws: Error.self) { try reader.readFrames(offset: 128_001, maxFrames: 1) }
    }

    @Test func preCancelledUploadCannotCreateANetworkRequest() throws {
        let transport = NativeRecognitionTransport()
        transport.cancel(requestId: "cancelled-before-dispatch")
        let request = RecognitionUploadRequest(requestId: "cancelled-before-dispatch", url: "https://127.0.0.1:1/must-not-run", bearerToken: "fixture", filePath: "/missing-fixture", timeoutSeconds: 1)
        do {
            _ = try transport.upload(request: request, progress: UploadProgress())
            Issue.record("A pre-cancelled upload must fail before touching its file or endpoint.")
        } catch PennelError.Cancelled {}
    }

    @Test func aacMeetingArchiveSupportsBoundedSeekAndExactEOF() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let archive = directory.appendingPathComponent("anderen.m4a")
        let settings: [String: Any] = [AVFormatIDKey: kAudioFormatMPEG4AAC, AVSampleRateKey: 16_000, AVNumberOfChannelsKey: 1, AVEncoderBitRateKey: 32_000]
        var writer: AVAudioFile? = try AVAudioFile(forWriting: archive, settings: settings, commonFormat: .pcmFormatFloat32, interleaved: false)
        let format = try #require(writer?.processingFormat)
        let audio = try #require(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 64_000))
        audio.frameLength = 64_000
        for index in 0..<64_000 { audio.floatChannelData![0][index] = 0.1 }
        try writer?.write(from: audio)
        try writer?.write(from: audio)
        writer = nil

        let reader = NativeRecognitionReader(archive: archive)
        let info = try reader.info()
        try #require(info.totalFrames > 64_000)
        let first = try reader.readFrames(offset: 0, maxFrames: 64_000)
        #expect(first.samples.count == 64_000)
        #expect(!first.eof)
        let tail = try reader.readFrames(offset: info.totalFrames - 137, maxFrames: 64_000)
        #expect(tail.samples.count == 137)
        let finiteSamples = tail.samples.allSatisfy(\.isFinite)
        #expect(finiteSamples)
        #expect(tail.eof)
        let end = try reader.readFrames(offset: info.totalFrames, maxFrames: 64_000)
        #expect(end.samples.isEmpty)
        #expect(end.eof)
    }

    @Test func changedArchiveCannotBeAppliedAgainstAPinnedDecoder() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let archive = directory.appendingPathComponent("fixture.caf")
        let format = try #require(AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 16_000, channels: 1, interleaved: false))
        let buffer = try #require(AVAudioPCMBuffer(pcmFormat: format, frameCapacity: 1_000))
        buffer.frameLength = 1_000
        for index in 0..<1_000 { buffer.floatChannelData![0][index] = 0.1 }
        var writer: AVAudioFile? = try AVAudioFile(forWriting: archive, settings: format.settings)
        try writer?.write(from: buffer)
        writer = nil
        let reader = NativeRecognitionReader(archive: archive)
        _ = try reader.info()
        for index in 0..<1_000 { buffer.floatChannelData![0][index] = 0.2 }
        writer = try AVAudioFile(forWriting: archive, settings: format.settings)
        try writer?.write(from: buffer)
        writer = nil
        #expect(abs(try reader.readFrames(offset: 0, maxFrames: 1_000).samples[0] - 0.1) < 0.0001)
        do { _ = try reader.info(); Issue.record("A changed original archive must invalidate the job.") }
        catch PennelError.Cloud(let code, _) { #expect(code == "speaker_stale") }
    }

    @Test func conflictFilesUseTheSharedNamingConventionAndExcludeSymlinks() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let conflict = directory.appendingPathComponent("notes (conflict laptop 2026-10-27).md")
        try Data("fixture".utf8).write(to: conflict)
        try Data("fixture".utf8).write(to: directory.appendingPathComponent("notes.md"))
        try FileManager.default.createSymbolicLink(at: directory.appendingPathComponent("meeting (conflict link).md"), withDestinationURL: conflict)
        #expect(PennelNativeRuntime.conflictURLs(directory).map(\.lastPathComponent) == [conflict.lastPathComponent])
    }

    @MainActor @Test func reconnectSnapshotIsMonotonicAndContainsNoNativeSecrets() throws {
        let first = try BridgeJSON.object(PennelNativeRuntime.shared.bootstrap())
        let second = try BridgeJSON.object(PennelNativeRuntime.shared.getSnapshot())
        let firstRevision = try #require(first["revision"] as? NSNumber).uint64Value
        let secondRevision = try #require(second["revision"] as? NSNumber).uint64Value
        #expect(firstRevision > 0)
        #expect(secondRevision >= firstRevision)
        #expect(second["platform"] as? String == "ios")
        #expect(second["ready"] as? Bool == true)
        #expect(second["settings"] is [String: Any])
        #expect(second["capabilities"] is [String: Any])
        let encoded = try BridgeJSON.encode(second)
        for field in ["bearer_token", "refresh_token", "vault_key", "device_code", "samples", "embedding"] {
            #expect(!encoded.contains("\"\(field)\""))
        }
    }

    @MainActor @Test func unknownCommandsFailRatherThanClaimSuccess() async throws {
        do {
            _ = try await PennelNativeRuntime.shared.invoke(method: "fixture.invalid", payload: "{}")
            Issue.record("Unknown native operations must fail.")
        } catch let error as PennelBridgeError { #expect(error.code == "unsupported_operation") }
    }

    @MainActor @Test func invalidSettingsDoNotReplaceSavedChoices() async throws {
        let before = try BridgeJSON.object(PennelNativeRuntime.shared.getSnapshot())["settings"] as? [String: Any]
        do {
            _ = try await PennelNativeRuntime.shared.invoke(method: "settings.update", payload: "{\"settings\":{\"mode\":\"invalid-mode\"}}")
            Issue.record("Invalid settings must fail.")
        } catch {}
        let after = try BridgeJSON.object(PennelNativeRuntime.shared.getSnapshot())["settings"] as? [String: Any]
        #expect(before?["mode"] as? String == after?["mode"] as? String)
    }

    @MainActor @Test func importedCloudSettingDoesNotAuthorizeAutomaticUploads() async throws {
        _ = try PennelNativeRuntime.shared.bootstrap()
        let model = AppModel.shared
        let original = model.settings
        let key = "pennel.mobile.cloudSpeakerAutomaticConsent"
        let savedConsent = UserDefaults.standard.object(forKey: key)
        defer {
            try? model.engine.updateSettings(settings: original)
            model.reloadSettings()
            if let savedConsent { UserDefaults.standard.set(savedConsent, forKey: key) }
            else { UserDefaults.standard.removeObject(forKey: key) }
        }
        model.update { $0.diarization = true; $0.speakerRecognition = .cloud }
        UserDefaults.standard.removeObject(forKey: key)
        #expect(!PennelNativeRuntime.automaticRecognitionAllowed(model.settings, consent: false))
        let before = try BridgeJSON.object(PennelNativeRuntime.shared.getSnapshot())["settings"] as? [String: Any]
        #expect(before?["diarization"] as? Bool == false)
        let nextMode = model.settings.mode == .fast ? "clean" : "fast"
        _ = try await PennelNativeRuntime.shared.invoke(method: "settings.update", payload: "{\"settings\":{\"mode\":\"\(nextMode)\"}}")
        let after = try BridgeJSON.object(PennelNativeRuntime.shared.getSnapshot())["settings"] as? [String: Any]
        #expect(after?["mode"] as? String == nextMode)
        #expect(after?["diarization"] as? Bool == false)
        #expect(!UserDefaults.standard.bool(forKey: key))
    }

    @MainActor @Test func failedProviderSavePreservesItsCredential() async throws {
        _ = try PennelNativeRuntime.shared.bootstrap()
        let model = AppModel.shared
        let original = model.settings
        let provider = newAccount(kind: .custom, existing: original.providers)
        var fixture = original
        fixture.providers.append(provider)
        try model.engine.updateSettings(settings: fixture)
        model.reloadSettings()
        try #require(Keychain.set(key: "synthetic-fixture", for: provider.id))
        let settingsURL = AppPaths.dataDirectory.appendingPathComponent("settings.json")
        let saved = try Data(contentsOf: settingsURL)
        try FileManager.default.removeItem(at: settingsURL)
        try FileManager.default.createDirectory(at: settingsURL, withIntermediateDirectories: false)
        defer {
            try? FileManager.default.removeItem(at: settingsURL)
            try? saved.write(to: settingsURL, options: .atomic)
            try? model.engine.updateSettings(settings: original)
            model.reloadSettings()
            Keychain.set(key: nil, for: provider.id)
        }
        let payload = try BridgeJSON.encode(["provider_id": provider.id])
        do {
            _ = try await PennelNativeRuntime.shared.invoke(method: "providers.remove", payload: payload)
            Issue.record("A failed provider save must reject.")
        } catch {}
        #expect(model.engine.settings().providers.contains(where: { $0.id == provider.id }))
        #expect(Keychain.hasKey(for: provider.id))
    }

    @MainActor @Test func staleModelVerificationCannotVerifyAnotherModel() async throws {
        _ = try PennelNativeRuntime.shared.bootstrap()
        let model = AppModel.shared
        let original = model.settings
        var provider = newAccount(kind: .custom, existing: original.providers)
        provider.baseUrl = "https://example.invalid"
        var fixture = original
        fixture.providers.append(provider)
        fixture.dictationStt = ModelChoice(providerId: provider.id, model: "fixture-current-model")
        try model.engine.updateSettings(settings: fixture)
        model.reloadSettings()
        defer { try? model.engine.updateSettings(settings: original); model.reloadSettings() }
        let payload = try BridgeJSON.encode(["provider_id": provider.id, "task": "dictation_stt", "verify_task": true, "expected_model": "fixture-obsolete-model"])
        do {
            _ = try await PennelNativeRuntime.shared.invoke(method: "providers.test", payload: payload)
            Issue.record("A stale verification request must not test a different model.")
        } catch let error as PennelBridgeError { #expect(error.code == "provider_changed") }
    }
}
