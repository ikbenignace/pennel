import Foundation
import OSLog

enum KeyEdge { case down, up }

enum NativeHotkeyTrigger: String {
    case fn, rightOption = "right_option", rightCommand = "right_command"
    case rightControl = "right_control", controlSuper = "control_super"
}

struct MicUser: Hashable, Sendable {
    let bundleId: String
    let isCall: Bool
    let isPlaying: Bool
}

struct NoInputDevice: LocalizedError {
    var errorDescription: String? { "Geen bruikbare microfoon gevonden" }
}

final class Diagnostics {
    private let logger: Logger
    init(_ category: String) { logger = Logger(subsystem: "dev.pennel.Pennel", category: category) }
    func error(_ message: String) { logger.error("\(message, privacy: .public)") }
    func notice(_ message: String) { logger.notice("\(message, privacy: .public)") }
}

func onMain<T>(_ work: () throws -> T) rethrows -> T {
    if Thread.isMainThread { return try work() }
    return try DispatchQueue.main.sync(execute: work)
}

func bridgeString(_ string: String) -> UnsafeMutablePointer<CChar>? { strdup(string) }

func bridgeJSON(_ value: Any) -> UnsafeMutablePointer<CChar>? {
    guard JSONSerialization.isValidJSONObject(value),
          let data = try? JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]),
          let string = String(data: data, encoding: .utf8) else { return bridgeString("null") }
    return bridgeString(string)
}

func decodeString(_ string: UnsafePointer<CChar>?) -> String? { string.map(String.init(cString:)) }
