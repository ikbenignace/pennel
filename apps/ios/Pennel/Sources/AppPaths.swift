import Foundation

/// The core keeps file locks and SQLite open, so it lives in the app's own container. A suspended app holding a
/// lock in the shared App Group container is killed by iOS (`0xdead10cc`); the group only carries handoff files.
enum AppPaths {
    static let appGroup = "group.app.pennel"

    static var dataDirectory: URL {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Pennel", isDirectory: true)
    }

    /// Visible in the Files app under "Op mijn iPhone › Pennel".
    static var meetingsDirectory: URL {
        FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Meetings", isDirectory: true)
    }
}
