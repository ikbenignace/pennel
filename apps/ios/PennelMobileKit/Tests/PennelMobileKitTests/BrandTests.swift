import Foundation
import Testing
@testable import PennelMobileKit

struct BrandTests {
    private static var brandMd: String {
        get throws {
            var url = URL(fileURLWithPath: #filePath)
            for _ in 0..<6 { url.deleteLastPathComponent() }
            return try String(contentsOf: url.appendingPathComponent("assets/brand/brand.md"), encoding: .utf8)
        }
    }

    private static func hex(_ value: Substring) -> UInt32? {
        UInt32(value.trimmingCharacters(in: CharacterSet(charactersIn: "` #")), radix: 16)
    }

    @Test func brand_tokens_match_brand_md() throws {
        var parsed: [String: (light: UInt32, dark: UInt32)] = [:]
        for line in try Self.brandMd.split(separator: "\n") where line.hasPrefix("| ") {
            let cells = line.split(separator: "|", omittingEmptySubsequences: false).dropFirst().dropLast()
                .map { $0.trimmingCharacters(in: .whitespaces) }
            guard cells.count == 3, let light = Self.hex(Substring(cells[1])), let dark = Self.hex(Substring(cells[2])) else { continue }
            parsed[cells[0]] = (light, dark)
        }

        #expect(parsed.count == 11)
        #expect(Set(parsed.keys) == Set(Brand.roles.keys))
        for (role, values) in parsed {
            let token = try #require(Brand.roles[role], "missing role \(role)")
            #expect(token.light == values.light, "\(role) light")
            #expect(token.dark == values.dark, "\(role) dark")
        }

        let text = try Self.brandMd
        #expect(text.contains("Paper `#FCFBF8`") && Brand.paper == 0xFCFBF8)
        #expect(text.contains("Ink `#2A2723`") && Brand.ink == 0x2A2723)
        #expect(text.contains("Soft wave `#F2B48E`") && Brand.softWave == 0xF2B48E)
        #expect(Brand.pillBackground == Brand.ink && Brand.pillText == Brand.paper && Brand.pillIndicator == Brand.softWave)
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
