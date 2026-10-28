import Foundation

public struct HandoffConfiguration: Equatable, Sendable {
    public let appGroup: String
    public let namespace: String

    public init(info: [String: Any]) {
        appGroup = Self.identifier(in: info, key: "PennelAppGroupIdentifier", fallback: "group.app.pennel", invalid: "group.app.pennel.invalid")
        namespace = Self.identifier(in: info, key: "PennelHandoffNamespace", fallback: "app.pennel.handoff", invalid: "app.pennel.invalid.handoff")
    }

    public static var current: HandoffConfiguration { HandoffConfiguration(info: Bundle.main.infoDictionary ?? [:]) }

    private static func identifier(in info: [String: Any], key: String, fallback: String, invalid: String) -> String {
        guard let value = info[key] else { return fallback }
        guard let value = value as? String, !value.isEmpty,
              value.allSatisfy({ $0.isASCII && ($0.isLetter || $0.isNumber || ".-_".contains($0)) })
        else { return invalid }
        return value
    }
}
