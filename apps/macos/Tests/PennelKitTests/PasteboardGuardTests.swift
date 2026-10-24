import Foundation
import PennelKit
import XCTest

private final class FakePasteboard: PasteboardLike {
    var changeCount = 0
    var items: [[String: Data]] = []

    func snapshot() -> [[String: Data]] { items }

    func restore(_ items: [[String: Data]]) {
        self.items = items
        changeCount += 1
    }

    func setString(_ s: String) {
        items = [["public.utf8-plain-text": Data(s.utf8)]]
        changeCount += 1
    }
}

final class PasteboardGuardTests: XCTestCase {
    func testRestoresOriginalItemsIncludingImages() {
        let pb = FakePasteboard()
        let image = [["public.png": Data([0x89, 0x50])]]
        pb.items = image
        let token = PasteboardGuard.begin(pb, text: "hallo")
        XCTAssertEqual(pb.items, [["public.utf8-plain-text": Data("hallo".utf8)]])
        PasteboardGuard.end(pb, token)
        XCTAssertEqual(pb.items, image)
    }

    func testDoesNotRestoreWhenUserCopiedMeanwhile() {
        let pb = FakePasteboard()
        pb.items = [["public.utf8-plain-text": Data("oud".utf8)]]
        let token = PasteboardGuard.begin(pb, text: "hallo")
        pb.setString("nieuw van gebruiker")
        PasteboardGuard.end(pb, token)
        XCTAssertEqual(pb.items, [["public.utf8-plain-text": Data("nieuw van gebruiker".utf8)]])
    }
}
