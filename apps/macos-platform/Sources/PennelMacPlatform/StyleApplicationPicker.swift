import AppKit
import Foundation
import UniformTypeIdentifiers

struct StyleApplicationSelection: Equatable, Sendable {
    let id: String
    let name: String
}

enum StyleApplicationPickerError: LocalizedError {
    case invalidApplication

    var errorDescription: String? { "Deze app heeft geen bruikbare app-ID. Kies een andere app." }
}

func styleApplicationSelection(bundleID: String?, name: String?, fallbackName: String) throws -> StyleApplicationSelection {
    guard let id = bundleID, !id.isEmpty, id.utf8.count <= 255,
          id.utf8.allSatisfy({ (65...90).contains($0) || (97...122).contains($0) || (48...57).contains($0) || $0 == 45 || $0 == 46 }),
          id != ".", id != ".." else { throw StyleApplicationPickerError.invalidApplication }
    let preferred = name?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
    let displayName = preferred.isEmpty ? fallbackName : preferred
    guard !displayName.isEmpty, displayName.utf8.count <= 255,
          !displayName.unicodeScalars.contains(where: CharacterSet.controlCharacters.contains) else {
        throw StyleApplicationPickerError.invalidApplication
    }
    return StyleApplicationSelection(id: id, name: displayName)
}

@MainActor
func pickStyleApplication() throws -> StyleApplicationSelection? {
    let panel = NSOpenPanel()
    panel.title = "App toevoegen…"
    panel.prompt = "Toevoegen"
    panel.allowedContentTypes = [.applicationBundle]
    panel.canChooseFiles = true
    panel.canChooseDirectories = false
    panel.allowsMultipleSelection = false
    panel.treatsFilePackagesAsDirectories = false
    panel.directoryURL = URL(fileURLWithPath: "/Applications", isDirectory: true)
    guard panel.runModal() == .OK, let url = panel.url else { return nil }
    guard url.isFileURL, url.pathExtension.lowercased() == "app", let bundle = Bundle(url: url) else {
        throw StyleApplicationPickerError.invalidApplication
    }
    return try styleApplicationSelection(
        bundleID: bundle.bundleIdentifier,
        name: bundle.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String
            ?? bundle.object(forInfoDictionaryKey: "CFBundleName") as? String,
        fallbackName: url.deletingPathExtension().lastPathComponent
    )
}
