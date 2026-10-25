import Foundation
import PennelCore
import XCTest

final class EngineSmokeTests: XCTestCase {
    private func makeEngine() throws -> PennelEngine {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        return try PennelEngine.newPreview(dataDir: dir.path)
    }

    func testEngineCreatesDefaultSettings() throws {
        let settings = try makeEngine().settings()
        XCTAssertEqual(settings.mode, .fast)
        XCTAssertEqual(settings.hotkey, .fn)
        XCTAssertEqual(settings.appearance, .system)
        XCTAssertEqual(settings.onboarding.status, .pending)
        XCTAssertFalse(settings.dictationPadAutoPaste)
    }

    func testSilenceIsNoSpeech() throws {
        let engine = try makeEngine()
        let request = DictationRequest(
            samples: [Float](repeating: 0, count: 16000),
            sampleRate: 16000,
            appBundleId: nil,
            keys: ApiKeys(byProvider: ["groq": "k"])
        )
        XCTAssertThrowsError(try engine.dictate(request: request)) { error in
            XCTAssertEqual(error as? PennelError, .NoSpeech)
        }
    }

    func testPadDraftSurvivesEngineRestart() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: dir) }
        do {
            let engine = try PennelEngine.newPreview(dataDir: dir.path)
            try engine.saveDictationPad(text: "Een bewerkt concept")
            XCTAssertEqual(try engine.appendDictationPad(text: "en een dictaat."), "Een bewerkt concept en een dictaat.")
        }
        let restarted = try PennelEngine.newPreview(dataDir: dir.path)
        XCTAssertEqual(restarted.dictationPadText(), "Een bewerkt concept en een dictaat.")
    }

    func testPracticeDoesNotChangeNormalMode() throws {
        let engine = try makeEngine()
        var settings = engine.settings()
        settings.mode = .clean
        try engine.updateSettings(settings: settings)
        let request = DictationRequest(samples: [Float](repeating: 0, count: 16000), sampleRate: 16000, appBundleId: "dev.pennel.Pennel", keys: ApiKeys(byProvider: [:]))
        XCTAssertThrowsError(try engine.dictatePractice(request: request))
        XCTAssertEqual(engine.settings().mode, .clean)
    }
}
