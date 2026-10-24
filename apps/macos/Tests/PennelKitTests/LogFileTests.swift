import PennelKit
import XCTest

final class LogFileTests: XCTestCase {
    private func tempDir() -> URL {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }

    func testAppendsTimestampedLines() throws {
        let dir = tempDir()
        let log = LogFile(url: dir.appendingPathComponent("pennel.log"), maxBytes: 10_000)
        log.append("dictation", "klaar", at: Date(timeIntervalSince1970: 0))
        log.append("meeting", "gestart", at: Date(timeIntervalSince1970: 60))
        let text = try String(contentsOf: log.url, encoding: .utf8)
        let lines = text.split(separator: "\n")
        XCTAssertEqual(lines.count, 2)
        XCTAssertTrue(lines[0].hasSuffix(" [dictation] klaar"), String(lines[0]))
        XCTAssertTrue(lines[1].contains("[meeting] gestart"))
    }

    func testRotatesWhenFull() throws {
        let dir = tempDir()
        let log = LogFile(url: dir.appendingPathComponent("pennel.log"), maxBytes: 200)
        for i in 0..<20 { log.append("x", "regel \(i) " + String(repeating: "a", count: 20)) }
        let size = try FileManager.default.attributesOfItem(atPath: log.url.path)[.size] as? Int ?? 0
        XCTAssertLessThanOrEqual(size, 200)
        XCTAssertTrue(FileManager.default.fileExists(atPath: dir.appendingPathComponent("pennel.1.log").path))
        XCTAssertTrue(try String(contentsOf: log.url, encoding: .utf8).contains("regel 19"))
    }
}
