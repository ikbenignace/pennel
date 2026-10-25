import Foundation

public final class HandoffStore: @unchecked Sendable {
    public enum File: String, Sendable {
        case session = "session.json"
        case command = "command.json"
        case result = "result.json"
    }

    public let directory: URL

    private let encoder: JSONEncoder = {
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        encoder.dateEncodingStrategy = .millisecondsSince1970
        return encoder
    }()

    private let decoder: JSONDecoder = {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        decoder.dateDecodingStrategy = .millisecondsSince1970
        return decoder
    }()

    public init(directory: URL) {
        self.directory = directory
    }

    /// Writes `<name>.<unique>.tmp` and renames it over the target; rename is atomic, so readers see the old or the new file.
    public func write<T: Encodable>(_ value: T, as file: File) throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let target = directory.appendingPathComponent(file.rawValue)
        let temporary = directory.appendingPathComponent("\(file.rawValue).\(UUID().uuidString).tmp")
        try encoder.encode(value).write(to: temporary)
        guard rename(temporary.path, target.path) == 0 else {
            let code = errno
            try? FileManager.default.removeItem(at: temporary)
            throw POSIXError(POSIXErrorCode(rawValue: code) ?? .EIO)
        }
    }

    public func read<T: Decodable>(_ type: T.Type, as file: File) -> T? {
        guard let data = try? Data(contentsOf: directory.appendingPathComponent(file.rawValue)) else { return nil }
        return try? decoder.decode(type, from: data)
    }

    /// Removes the result only if it still belongs to `id`, never a newer dictation's.
    public func clearResult(id: UUID) {
        guard let current = read(HandoffResult.self, as: .result), current.id == id else { return }
        try? FileManager.default.removeItem(at: directory.appendingPathComponent(File.result.rawValue))
    }
}
