import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// **Delen**: the core's `export_meeting` to the pasteboard (formatted as HTML plus plain Markdown, like the
/// desktop's rich copy) or to the iOS share sheet, which replaces the Mac's Mail…/Deel….
enum MeetingShare {
    static func export(_ engine: PennelEngine, id: String, transcript: Bool) async throws -> MeetingExport {
        try await Task.detached { try engine.exportMeeting(id: id, includeTranscript: transcript) }.value
    }

    static func copy(_ export: MeetingExport, formatted: Bool) {
        var item: [String: Any] = [UTType.utf8PlainText.identifier: export.markdown]
        if formatted { item[UTType.html.identifier] = export.html }
        UIPasteboard.general.setItems([item])
    }

    @MainActor
    static func present(_ export: MeetingExport) {
        let controller = UIActivityViewController(activityItems: [export.markdown], applicationActivities: nil)
        controller.setValue(export.title, forKey: "subject")
        guard let root = UIApplication.shared.connectedScenes
            .compactMap({ ($0 as? UIWindowScene)?.keyWindow?.rootViewController }).first
        else { return }
        var top = root
        while let presented = top.presentedViewController { top = presented }
        controller.popoverPresentationController?.sourceView = top.view
        top.present(controller, animated: true)
    }
}
