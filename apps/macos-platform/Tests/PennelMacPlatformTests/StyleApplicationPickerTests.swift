import AppKit
import XCTest
@testable import PennelMacPlatform

final class StyleApplicationPickerTests: XCTestCase {
    @MainActor
    func testSheetUsesItsOwnerAndCancellationRestoresTheOriginalResponder() async throws {
        _ = NSApplication.shared
        let owner = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        owner.isReleasedWhenClosed = false
        let field = PickerFocusView(frame: owner.contentView!.bounds)
        owner.contentView!.addSubview(field)
        XCTAssertTrue(owner.makeFirstResponder(field))
        let panel = NSOpenPanel()
        let selected = try await pickStyleApplication(title: "Add app…", prompt: "Add", owner: owner, panel: panel) { shown, parent, finish in
            XCTAssertTrue(shown === panel)
            XCTAssertTrue(parent === owner)
            XCTAssertFalse(shown.canChooseDirectories)
            XCTAssertFalse(shown.allowsMultipleSelection)
            XCTAssertEqual(shown.title, "Add app…")
            owner.makeFirstResponder(nil)
            finish(.cancel)
        }
        XCTAssertNil(selected)
        XCTAssertTrue(owner.firstResponder === field)
        owner.close()
    }

    func testCancellingIgnoresAnUnusableSelection() throws {
        XCTAssertNil(try styleApplicationSelection(response: .cancel, url: URL(fileURLWithPath: "/missing.app")))
        XCTAssertThrowsError(try styleApplicationSelection(response: .OK, url: URL(string: "https://example.test/App.app")))
    }

    @MainActor
    func testPickerRelabelsWithoutReplacingItsSelectionOrRules() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        for (title, prompt) in [("Add app…", "Add"), ("App toevoegen…", "Toevoegen"), ("Add app…", "Add")] {
            applyStyleApplicationPickerLabels(panel, title: title, prompt: prompt)
            XCTAssertEqual(panel.title, title)
            XCTAssertEqual(panel.prompt, prompt)
            XCTAssertFalse(panel.canChooseDirectories)
            XCTAssertFalse(panel.allowsMultipleSelection)
        }
    }

    func testBundleIdentityKeepsCaseAndUsesDisplayName() throws {
        let app = try styleApplicationSelection(bundleID: "com.microsoft.VSCode", name: " Visual Studio Code ", fallbackName: "Code")
        XCTAssertEqual(app.id, "com.microsoft.VSCode")
        XCTAssertEqual(app.name, "Visual Studio Code")
    }

    func testMissingDisplayNameUsesAppFilename() throws {
        XCTAssertEqual(try styleApplicationSelection(bundleID: "dev.example.App", name: nil, fallbackName: "Example").name, "Example")
    }

    func testUnusableMetadataFailsWithoutCreatingARule() {
        for id in [nil, "", ".", "../App", "dev.app\n", "dev.app/example"] as [String?] {
            XCTAssertThrowsError(try styleApplicationSelection(bundleID: id, name: "App", fallbackName: "App"))
        }
        XCTAssertThrowsError(try styleApplicationSelection(bundleID: "dev.app", name: "App\nname", fallbackName: "App"))
    }
}

private final class PickerFocusView: NSView {
    override var acceptsFirstResponder: Bool { true }
}
