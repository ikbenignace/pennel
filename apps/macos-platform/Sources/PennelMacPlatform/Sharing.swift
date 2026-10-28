import AppKit
import Foundation

enum SharingAction: String { case mail, share }

func composeMailURL(text: String, title: String) -> URL? {
    var components = URLComponents()
    components.scheme = "mailto"
    components.queryItems = [URLQueryItem(name: "subject", value: title), URLQueryItem(name: "body", value: text)]
    return components.url
}

private var sharingPicker: NSSharingServicePicker?
private var mailService: NSSharingService?

@_cdecl("pennel_mac_share")
public func shareNative(_ rawText: UnsafePointer<CChar>?, _ rawTitle: UnsafePointer<CChar>?, _ rawAction: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>? {
    guard let action = SharingAction(rawValue: decodeString(rawAction) ?? "") else { return bridgeString("Unknown share action") }
    let text = decodeString(rawText) ?? ""
    let title = decodeString(rawTitle) ?? "Meeting"
    return onMain {
        switch action {
        case .mail:
            if let service = NSSharingService(named: .composeEmail), service.canPerform(withItems: [text]) {
                mailService = service
                service.subject = title
                service.perform(withItems: [text])
                return nil
            }
            guard let url = composeMailURL(text: text, title: title), NSWorkspace.shared.open(url) else {
                return bridgeString("No mail app is available to open a draft")
            }
            return nil
        case .share:
            guard let window = NSApplication.shared.keyWindow, let view = window.contentView else {
                return bridgeString("Open the meeting in Pennel to use the share menu")
            }
            let point = view.convert(window.mouseLocationOutsideOfEventStream, from: nil)
            let picker = NSSharingServicePicker(items: [text])
            sharingPicker = picker
            picker.show(relativeTo: NSRect(origin: point, size: .zero), of: view, preferredEdge: .minY)
            return nil
        }
    }
}
