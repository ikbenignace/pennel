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
private enum StyleApplicationPickerState {
    static weak var panel: NSOpenPanel?
}

@MainActor
func updateStyleApplicationPicker(title: String, prompt: String) {
    guard let panel = StyleApplicationPickerState.panel else { return }
    applyStyleApplicationPickerLabels(panel, title: title, prompt: prompt)
}

@MainActor
func applyStyleApplicationPickerLabels(_ panel: NSOpenPanel, title: String, prompt: String) {
    panel.title = title
    panel.prompt = prompt
}

@MainActor
func pickStyleApplication(title: String, prompt: String) throws -> StyleApplicationSelection? {
    let panel = NSOpenPanel()
    applyStyleApplicationPickerLabels(panel, title: title, prompt: prompt)
    StyleApplicationPickerState.panel = panel
    defer { if StyleApplicationPickerState.panel === panel { StyleApplicationPickerState.panel = nil } }
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
