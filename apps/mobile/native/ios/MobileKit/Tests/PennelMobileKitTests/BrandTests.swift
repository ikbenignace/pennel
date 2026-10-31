import Foundation
import Testing
@testable import PennelMobileKit

struct BrandTests {
    private struct TokenFile: Decodable {
        struct Pair: Decodable { let light: String, dark: String }
        let colors: [String: Pair]
    }

    private static func brandFile(_ relative: String) throws -> URL {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<8 { url.deleteLastPathComponent() }
        let file = url.appendingPathComponent(relative)
        try #require(FileManager.default.fileExists(atPath: file.path), "brand source not found at \(file.path)")
        return file
    }

    private static func parse(_ hex: String) -> (rgb: UInt32, alpha: Double)? {
        let digits = hex.trimmingCharacters(in: CharacterSet(charactersIn: "#"))
        guard digits.count == 6 || digits.count == 8, let value = UInt32(digits, radix: 16) else { return nil }
        return digits.count == 6 ? (value, 1) : (value >> 8, Double(value & 0xFF) / 255)
    }

    @Test func brand_tokens_match_brand_md() throws {
        let tokens = try JSONDecoder().decode(TokenFile.self, from: Data(contentsOf: Self.brandFile("assets/brand/tokens.json")))

        #expect(tokens.colors.count >= 30)
        #expect(Set(tokens.colors.keys) == Set(Brand.colors.keys))
        for (name, pair) in tokens.colors {
            let token = try #require(Brand.colors[name], "missing token \(name)")
            let light = try #require(Self.parse(pair.light), "bad hex \(pair.light)")
            let dark = try #require(Self.parse(pair.dark), "bad hex \(pair.dark)")
            #expect(token.light == light.rgb, "\(name) light")
            #expect(token.dark == dark.rgb, "\(name) dark")
            #expect(abs(token.lightAlpha - light.alpha) < 0.001, "\(name) light alpha")
            #expect(abs(token.darkAlpha - dark.alpha) < 0.001, "\(name) dark alpha")
        }

        let text = try String(contentsOf: Self.brandFile("assets/brand/brand.md"), encoding: .utf8)
        #expect(text.contains("Paper `#FCFBF8`") && Brand.paper == 0xFCFBF8)
        #expect(text.contains("Ink `#2A2723`") && Brand.ink == 0x2A2723)
        #expect(text.contains("Soft wave `#F2B48E`") && Brand.softWave == 0xF2B48E)
        #expect(Brand.brandPaper.light == Brand.paper && Brand.brandInk.light == Brand.ink && Brand.wave.light == Brand.softWave)
        #expect(Brand.pillBackground.dark == Brand.ink && Brand.pillText.dark == Brand.paper && Brand.pillAccent.dark == Brand.softWave)
    }

    @Test func wave_has_five_bars_that_follow_the_level() {
        #expect(Brand.Wave.barCount == 5)
        let quiet = Brand.Wave.barHeights(level: 0, height: 100)
        let loud = Brand.Wave.barHeights(level: 1, height: 100)
        #expect(quiet.count == 5 && loud.count == 5)
        #expect(zip(quiet, loud).allSatisfy { $0 <= $1 })
        #expect(loud.max() == 100)
    }
}
