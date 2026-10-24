import PennelKit
import XCTest

final class LanguageNamesTests: XCTestCase {
    func testDutchNames() {
        XCTAssertEqual(LanguageNames.name("nl"), "Nederlands")
        XCTAssertEqual(LanguageNames.name("en"), "Engels")
    }

    func testUnknownCodeFallsBack() {
        XCTAssertEqual(LanguageNames.name("xx"), "xx")
    }

    func testListJoinsSortedNames() {
        XCTAssertEqual(LanguageNames.list(["en", "nl", "de"]), "Duits, Engels, Nederlands")
    }
}
