import AppKit
import PennelKit
import os

/// Writes to the unified log and to `~/Library/Logs/Pennel/pennel.log`, which the user can open from the menu.
/// Never log dictated or meeting text; lengths, timings and error messages only.
struct Diagnostics: Sendable {
    static let file = LogFile(
        url: FileManager.default.urls(for: .libraryDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Logs/Pennel/pennel.log")
    )

    private let category: String
    private let logger: Logger

    init(_ category: String) {
        self.category = category
        logger = Logger(subsystem: "dev.pennel.Pennel", category: category)
    }

    /// Unified log only: too chatty for the file.
    func debug(_ message: String) {
        logger.debug("\(message, privacy: .public)")
    }

    func info(_ message: String) {
        logger.info("\(message, privacy: .public)")
        Self.file.append(category, message)
    }

    func notice(_ message: String) {
        logger.notice("\(message, privacy: .public)")
        Self.file.append(category, message)
    }

    func error(_ message: String) {
        logger.error("\(message, privacy: .public)")
        Self.file.append(category, "FOUT " + message)
    }

    @MainActor
    static func reveal() {
        if !FileManager.default.fileExists(atPath: file.url.path) {
            file.append("app", "logboek aangemaakt")
        }
        NSWorkspace.shared.open(file.url)
    }
}
