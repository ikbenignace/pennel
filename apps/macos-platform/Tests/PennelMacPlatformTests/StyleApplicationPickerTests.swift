import AppKit
import XCTest
@testable import PennelMacPlatform

final class StyleApplicationPickerTests: XCTestCase {
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
