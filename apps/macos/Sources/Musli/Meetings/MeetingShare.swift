import AppKit
@preconcurrency import MusliCore

@MainActor
enum MeetingShare {
    static func copyFormatted(_ export: MeetingExport) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        let html = "<meta charset=\"utf-8\">" + export.html
        pasteboard.setString(html, forType: .html)
        pasteboard.setString(export.markdown, forType: .string)
    }

    static func copyMarkdown(_ export: MeetingExport) {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(export.markdown, forType: .string)
    }

    static func mail(_ export: MeetingExport) {
        guard let service = NSSharingService(named: .composeEmail) else { return }
        service.subject = export.title
        service.perform(withItems: [export.markdown])
    }

    static func share(_ export: MeetingExport, from view: NSView?) {
        let picker = NSSharingServicePicker(items: [export.markdown])
        guard let view = view ?? NSApp.keyWindow?.contentView else { return }
        picker.show(relativeTo: view.bounds, of: view, preferredEdge: .minY)
    }
}
