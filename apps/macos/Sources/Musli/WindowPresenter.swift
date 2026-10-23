import AppKit
import MusliKit
import SwiftUI

@MainActor
final class WindowPresenter {
    private var windows: [String: NSWindow] = [:]

    func show(id: String, title: String, size: NSSize, @ViewBuilder content: () -> some View) {
        if let window = windows[id] {
            window.makeKeyAndOrderFront(nil)
            NSApp.activate()
            return
        }
        let window = NSWindow(
            contentRect: NSRect(origin: .zero, size: size),
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        window.title = title
        window.titlebarAppearsTransparent = true
        window.isReleasedWhenClosed = false
        // The paper palette is light-only for now; without this, dark-mode controls draw light text on paper.
        window.appearance = NSAppearance(named: .aqua)
        window.backgroundColor = NSColor(Theme.paper)
        window.contentView = NSHostingView(rootView: content().font(.figtree(13)).foregroundStyle(Theme.ink))
        window.center()
        windows[id] = window
        window.makeKeyAndOrderFront(nil)
        NSApp.activate()
    }
}
