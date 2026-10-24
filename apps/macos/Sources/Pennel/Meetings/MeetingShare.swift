import AppKit
@preconcurrency import PennelCore

@MainActor
enum MeetingShare {
    static func copyFormatted(_ export: MeetingExport) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setData(Data(("<meta charset=\"utf-8\">" + export.html).utf8), forType: .html)
        pasteboard.setString(export.markdown, forType: .string)
    }

    static func copyMarkdown(_ export: MeetingExport) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(export.markdown, forType: .string)
    }

    /// Falls back to a `mailto:` link when no mail app offers the compose service.
    static func mail(_ export: MeetingExport) {
        if let service = NSSharingService(named: .composeEmail), service.canPerform(withItems: [export.markdown]) {
            service.subject = export.title
            service.perform(withItems: [export.markdown])
            return
        }
        var components = URLComponents()
        components.scheme = "mailto"
        components.queryItems = [URLQueryItem(name: "subject", value: export.title), URLQueryItem(name: "body", value: export.markdown)]
        if let url = components.url { NSWorkspace.shared.open(url) }
    }

    /// Anchored at the pointer, which is where the "Delen" menu was just clicked.
    static func share(_ export: MeetingExport) {
        guard let window = NSApp.keyWindow, let view = window.contentView else { return }
        let point = view.convert(window.mouseLocationOutsideOfEventStream, from: nil)
        let picker = NSSharingServicePicker(items: [export.markdown])
        picker.show(relativeTo: NSRect(origin: point, size: .zero), of: view, preferredEdge: .minY)
    }
}
