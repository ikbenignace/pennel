import Foundation
import Testing
@testable internal import PennelMobile

@MainActor
struct EngineSmokeTests {
    @Test func engine_opens_in_app_container_not_app_group() throws {
        let model = AppModel.shared
        let dataDir = AppPaths.dataDirectory.standardizedFileURL.path

        #expect(dataDir.hasPrefix(URL(fileURLWithPath: NSHomeDirectory()).standardizedFileURL.path))
        #expect(dataDir.hasSuffix("/Library/Application Support/Pennel"))
        #expect(FileManager.default.fileExists(atPath: dataDir))
        #expect(!model.engine.settings().providers.isEmpty)

        // The simulator can return nil for an unprovisioned group; then compare against the identifier itself.
        if let group = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: AppPaths.appGroup) {
            #expect(!dataDir.hasPrefix(group.standardizedFileURL.path))
        } else {
            #expect(!dataDir.contains(AppPaths.appGroup))
            #expect(!dataDir.contains("/Shared/AppGroup/"))
        }
    }

    @Test func meetings_dir_defaults_to_documents_meetings() {
        let documents = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        let expected = documents.appendingPathComponent("Meetings").standardizedFileURL.path

        #expect(AppModel.shared.engine.settings().meetingsDir == expected)
        #expect(AppModel.shared.settings.meetingsDir == expected)
        #expect(FileManager.default.fileExists(atPath: expected))
    }
}
