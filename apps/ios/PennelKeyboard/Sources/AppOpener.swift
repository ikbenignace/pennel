import UIKit

@MainActor
enum AppOpener {
    static func openSettings(from context: NSExtensionContext?) {
        guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
        context?.open(url, completionHandler: nil)
    }
}
