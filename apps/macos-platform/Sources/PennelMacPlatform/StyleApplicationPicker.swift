import AppKit
import Foundation
import UniformTypeIdentifiers

struct StyleApplicationSelection: Equatable, Sendable {
    let id: String
    let name: String
}

enum StyleApplicationPickerError: LocalizedError {
    case invalidApplication

    var errorDescription: String? { "This app has no usable app ID. Choose another app." }
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
func pickStyleApplication(
    title: String,
    prompt: String,
    owner: NSWindow,
    panel suppliedPanel: NSOpenPanel? = nil,
    present: ((NSOpenPanel, NSWindow, @escaping (NSApplication.ModalResponse) -> Void) -> Void)? = nil
) async throws -> StyleApplicationSelection? {
    guard StyleApplicationPickerState.panel == nil, owner.attachedSheet == nil else { return nil }
    let panel = suppliedPanel ?? NSOpenPanel()
    let previousResponder = owner.firstResponder
    applyStyleApplicationPickerLabels(panel, title: title, prompt: prompt)
    StyleApplicationPickerState.panel = panel
    defer {
        if StyleApplicationPickerState.panel === panel { StyleApplicationPickerState.panel = nil }
        if let view = previousResponder as? NSView, view.window === owner { owner.makeFirstResponder(view) }
    }
    panel.allowedContentTypes = [.applicationBundle]
    panel.canChooseFiles = true
    panel.canChooseDirectories = false
    panel.allowsMultipleSelection = false
    panel.treatsFilePackagesAsDirectories = false
    panel.directoryURL = URL(fileURLWithPath: "/Applications", isDirectory: true)
    let response = await withCheckedContinuation { continuation in
        let finish: (NSApplication.ModalResponse) -> Void = { continuation.resume(returning: $0) }
        if let present { present(panel, owner, finish) }
        else { panel.beginSheetModal(for: owner, completionHandler: finish) }
    }
    return try styleApplicationSelection(response: response, url: panel.url)
}

func styleApplicationSelection(response: NSApplication.ModalResponse, url: URL?) throws -> StyleApplicationSelection? {
    guard response == .OK, let url else { return nil }
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
