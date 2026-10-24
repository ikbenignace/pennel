import Foundation
import PennelCore
import XCTest

final class EngineSmokeTests: XCTestCase {
    private func makeEngine() throws -> PennelEngine {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        return try PennelEngine(dataDir: dir.path)
    }

    func testEngineCreatesDefaultSettings() throws {
        let settings = try makeEngine().settings()
        XCTAssertEqual(settings.mode, .clean)
        XCTAssertEqual(settings.hotkey, .fn)
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
}
