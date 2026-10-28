import Foundation
import Testing
@testable import Pennel

private final class FakeTranscriber: SystemTranscriber, @unchecked Sendable {
    private let lock = NSLock()
    private(set) var calls: [(count: Int, sampleRate: UInt32)] = []

    func transcribe(samples: [Float], sampleRate: UInt32, language: String?) throws -> String {
        lock.withLock { calls.append((samples.count, sampleRate)) }
        return "hallo wereld"
    }
}

private func tempEngine() throws -> PennelEngine {
    let dir = FileManager.default.temporaryDirectory.appendingPathComponent("pennel-test-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    return try PennelEngine.newPreview(dataDir: dir.path)
}

private func appleSpeechSettings(_ engine: PennelEngine) throws {
    try FirstRunDefaults.apply(to: engine, appleIntelligenceAvailable: false, hasKey: { _ in false })
}

private func tone(seconds: Double) -> [Float] {
    (0..<Int(16_000 * seconds)).map { 0.4 * sin(Float($0) * 0.05) }
}

struct DictationRunnerTests {
    @Test func runner_passes_16k_and_no_bundle_id() async throws {
        let engine = try tempEngine()
        let fake = FakeTranscriber()
        engine.setSystemTranscriber(transcriber: fake)
        try appleSpeechSettings(engine)

        let samples = tone(seconds: 1)
        let result = try await DictationRunner(engine: engine, keys: { ApiKeys(byProvider: [:]) }).run(samples: samples).get()

        #expect(fake.calls.count == 1)
        #expect(fake.calls.first?.sampleRate == 16_000)
        #expect(fake.calls.first?.count == samples.count)
        #expect(result.raw == "hallo wereld")
        let entry = try #require(try engine.history(limit: 10).first)
        #expect(entry.appBundleId == nil)
    }

    @Test func no_speech_has_an_english_diagnostic_and_stable_presentation_code() async throws {
        let engine = try tempEngine()
        engine.setSystemTranscriber(transcriber: FakeTranscriber())
        try appleSpeechSettings(engine)

        let result = await DictationRunner(engine: engine, keys: { ApiKeys(byProvider: [:]) })
            .run(samples: [Float](repeating: 0, count: 16_000))

        guard case .failure(let error) = result else { Issue.record("expected a failure"); return }
        #expect(error == .NoSpeech)
        #expect(ErrorText.message(error) == "No speech recognized. Try again.")
        #expect(ErrorText.failureCode(error) == "no_speech")
    }

    @Test func first_run_defaults_pick_apple_providers_without_overwriting() throws {
        let fresh = try tempEngine()
        try FirstRunDefaults.apply(to: fresh, appleIntelligenceAvailable: true, hasKey: { _ in false })
        var settings = fresh.settings()
        func kind(_ choice: ModelChoice) -> ProviderKind? { settings.providers.first { $0.id == choice.providerId }?.kind }
        #expect(kind(settings.dictationStt) == .appleSpeech)
        #expect(kind(settings.meetingStt) == .appleSpeech)
        #expect(kind(settings.llm) == .appleIntelligence)
        #expect(settings.mode == .clean)

        let unavailable = try tempEngine()
        try FirstRunDefaults.apply(to: unavailable, appleIntelligenceAvailable: false, hasKey: { _ in false })
        #expect(unavailable.settings().mode == .fast)
        #expect(unavailable.settings().providers.contains { $0.kind == .appleIntelligence } == false)

        let groq = try #require(settings.providers.first { $0.kind == .groq } ?? fresh.settings().providers.first { $0.kind == .groq })
        settings.dictationStt = ModelChoice(providerId: groq.id, model: "whisper-large-v3")
        settings.llm = ModelChoice(providerId: groq.id, model: "llama")
        settings.mode = .clean
        try fresh.updateSettings(settings: settings)
        try FirstRunDefaults.apply(to: fresh, appleIntelligenceAvailable: true, hasKey: { _ in false })
        let kept = fresh.settings()
        #expect(kept.dictationStt == ModelChoice(providerId: groq.id, model: "whisper-large-v3"))
        #expect(kept.llm == ModelChoice(providerId: groq.id, model: "llama"))
        #expect(kept.mode == .clean)
    }

    @Test func first_run_defaults_keep_a_keyed_factory_llm() throws {
        let engine = try tempEngine()
        let before = engine.settings()
        try FirstRunDefaults.apply(to: engine, appleIntelligenceAvailable: true, hasKey: { _ in true })
        let after = engine.settings()
        #expect(after.llm == before.llm)
        #expect(after.providers.first { $0.id == after.llm.providerId }?.kind == .groq)
        #expect(after.mode == before.mode)
    }
}
