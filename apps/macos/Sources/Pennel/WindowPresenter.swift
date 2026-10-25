import AppKit
@preconcurrency import PennelCore
import PennelKit
import SwiftUI

enum WindowLayout {
    static func constrained(size: NSSize, to screen: NSRect, margin: CGFloat = 24) -> NSSize {
        NSSize(width: min(size.width, max(1, screen.width - margin * 2)), height: min(size.height, max(1, screen.height - margin * 2)))
    }
}

@MainActor
enum NativeAppearance {
    static func apply(_ preference: AppearancePreference) {
        let appearance: NSAppearance? = switch preference {
        case .system: nil
        case .light: NSAppearance(named: .aqua)
        case .dark: NSAppearance(named: .darkAqua)
        }
        NSApp.appearance = appearance
        for window in NSApp.windows {
            window.appearance = appearance
            if window is NSPanel { continue }
            window.backgroundColor = window.identifier?.rawValue == "onboarding" ? .clear : NSColor(Theme.paper)
        }
    }
}

@MainActor
final class WindowPresenter: NSObject, NSWindowDelegate {
    enum Style { case workspace, onboarding }
    private var windows: [String: NSWindow] = [:]
    private var closeActions: [ObjectIdentifier: () -> Void] = [:]
    private var resignActions: [ObjectIdentifier: () -> Void] = [:]

    func show(id: String, title: String, size: NSSize, style: Style = .workspace, onHide: (() -> Void)? = nil, onResign: (() -> Void)? = nil, @ViewBuilder content: () -> some View) {
        if let window = windows[id] {
            if let screen = window.screen?.visibleFrame ?? NSScreen.main?.visibleFrame {
                window.minSize = WindowLayout.constrained(size: style == .onboarding ? NSSize(width: 580, height: 540) : NSSize(width: 760, height: 520), to: screen)
                if !screen.contains(window.frame) { fit(window, to: screen) }
            }
            window.makeKeyAndOrderFront(nil)
            NSApp.activate()
            return
        }
        let screen = NSScreen.main?.visibleFrame
        let requested = screen.map { WindowLayout.constrained(size: size, to: $0) } ?? size
        let window = PennelWindow(
            contentRect: NSRect(origin: .zero, size: requested),
            styleMask: style == .onboarding ? [.borderless, .closable, .resizable] : [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = title
        window.identifier = NSUserInterfaceItemIdentifier(id)
        window.titlebarAppearsTransparent = true
        window.isReleasedWhenClosed = false
        window.appearance = NSApp.appearance
        window.backgroundColor = style == .onboarding ? .clear : NSColor(Theme.paper)
        window.isOpaque = style != .onboarding
        window.isMovableByWindowBackground = style == .onboarding
        window.titleVisibility = style == .onboarding ? .hidden : .visible
        let minimum = style == .onboarding ? NSSize(width: 580, height: 540) : NSSize(width: 760, height: 520)
        window.minSize = screen.map { WindowLayout.constrained(size: minimum, to: $0) } ?? minimum
        window.delegate = self
        if let onHide { closeActions[ObjectIdentifier(window)] = onHide }
        if let onResign { resignActions[ObjectIdentifier(window)] = onResign }
        window.contentView = NSHostingView(rootView: content().font(.figtree(13)).foregroundStyle(Theme.ink))
        if style == .onboarding {
            window.contentView?.wantsLayer = true
            window.contentView?.layer?.cornerRadius = 12
            window.contentView?.layer?.masksToBounds = true
        }
        if let screen { fit(window, to: screen) } else { window.center() }
        windows[id] = window
        window.makeKeyAndOrderFront(nil)
        NSApp.activate()
    }

    func hide(_ id: String) {
        guard let window = windows[id] else { return }
        closeActions[ObjectIdentifier(window)]?()
        window.orderOut(nil)
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        closeActions[ObjectIdentifier(sender)]?()
        sender.orderOut(nil)
        return false
    }

    func windowDidResignKey(_ notification: Notification) {
        if let window = notification.object as? NSWindow { resignActions[ObjectIdentifier(window)]?() }
    }

    private func fit(_ window: NSWindow, to screen: NSRect) {
        let size = WindowLayout.constrained(size: window.frame.size, to: screen)
        window.setFrame(NSRect(x: screen.midX - size.width / 2, y: screen.midY - size.height / 2, width: size.width, height: size.height), display: false)
    }
}

private final class PennelWindow: NSWindow {
    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }
    override func cancelOperation(_ sender: Any?) { performClose(sender) }
}
