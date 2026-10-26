import UIKit

/// The only place the keyboard opens another app or the Settings app. Keep it isolated: if App Review objects to
/// guideline 4.4.1 (spec R1), `isEnabled` goes to false and the pad shows the manual fallback instead.
@MainActor
enum AppOpener {
    static let isEnabled = true

    static let sessionURL = URL(string: "pennel://sessie/start")!
    static let settingsURL = URL(string: "app-settings:")!

    /// Returns false when nothing in the responder chain could open it.
    static func openSession(from responder: UIResponder) -> Bool {
        isEnabled && open(sessionURL, from: responder)
    }

    static func openSettings(from responder: UIResponder) -> Bool {
        open(settingsURL, from: responder)
    }

    private static func open(_ url: URL, from responder: UIResponder) -> Bool {
        // `UIApplication.shared` is unavailable in extensions; the application sits at the end of the responder chain.
        let selector = NSSelectorFromString("open:options:completionHandler:")
        typealias Open = @convention(c) (AnyObject, Selector, URL, [UIApplication.OpenExternalURLOptionsKey: Any], ((Bool) -> Void)?) -> Void
        var next: UIResponder? = responder
        while let current = next {
            if current.responds(to: selector), let implementation = current.method(for: selector) {
                unsafeBitCast(implementation, to: Open.self)(current, selector, url, [:], nil)
                return true
            }
            next = current.next
        }
        return false
    }
}
