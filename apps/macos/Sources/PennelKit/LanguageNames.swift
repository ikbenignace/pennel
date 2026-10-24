import Foundation

public enum LanguageNames {
    public static let common = ["nl", "en", "fr", "de", "es", "it", "pt", "pl"]

    private static let dutch = Locale(identifier: "nl")

    public static func name(_ code: String) -> String {
        guard let name = dutch.localizedString(forLanguageCode: code), name != code else { return code }
        return name.prefix(1).uppercased() + name.dropFirst()
    }

    public static func list(_ codes: [String]) -> String {
        codes.map(name).sorted().joined(separator: ", ")
    }
}
