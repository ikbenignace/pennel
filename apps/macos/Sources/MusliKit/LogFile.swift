import Foundation

/// A plain-text log the user can open and share; rotates to `<name>.1.log` when it grows past `maxBytes`.
public final class LogFile: @unchecked Sendable {
    public let url: URL
    private let maxBytes: Int
    private let lock = NSLock()
    private let formatter: ISO8601DateFormatter = {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        formatter.timeZone = .current
        return formatter
    }()

    public init(url: URL, maxBytes: Int = 1_000_000) {
        self.url = url
        self.maxBytes = maxBytes
    }

    public func append(_ category: String, _ message: String, at date: Date = Date()) {
        let line = "\(formatter.string(from: date)) [\(category)] \(message.replacingOccurrences(of: "\n", with: " "))\n"
        lock.withLock {
            let fm = FileManager.default
            try? fm.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            let size = (try? fm.attributesOfItem(atPath: url.path)[.size] as? Int) ?? 0
            if size + line.utf8.count > maxBytes {
                let previous = url.deletingPathExtension().appendingPathExtension("1.log")
                try? fm.removeItem(at: previous)
                try? fm.moveItem(at: url, to: previous)
            }
            guard let handle = try? FileHandle(forWritingTo: url) else {
                try? Data(line.utf8).write(to: url)
                return
            }
            defer { try? handle.close() }
            _ = try? handle.seekToEnd()
            try? handle.write(contentsOf: Data(line.utf8))
        }
    }
}
