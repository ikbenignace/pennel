import PennelKit
import XCTest

final class SecretCacheTests: XCTestCase {
    func testLoadsEachAccountOnce() {
        var loads: [String] = []
        let cache = SecretCache { account in
            loads.append(account)
            return account == "groq" ? "k1" : nil
        }
        XCTAssertEqual(cache.get("groq"), "k1")
        XCTAssertEqual(cache.get("groq"), "k1")
        XCTAssertNil(cache.get("local"))
        XCTAssertNil(cache.get("local"))
        XCTAssertEqual(loads, ["groq", "local"])
    }

    func testStoreUpdatesWithoutReloading() {
        var loads = 0
        let cache = SecretCache { _ in
            loads += 1
            return "old"
        }
        cache.store("new", for: "groq")
        XCTAssertEqual(cache.get("groq"), "new")
        cache.store(nil, for: "groq")
        XCTAssertNil(cache.get("groq"))
        XCTAssertEqual(loads, 0)
    }
}
