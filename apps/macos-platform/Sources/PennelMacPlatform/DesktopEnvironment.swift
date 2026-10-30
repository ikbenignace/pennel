import AppKit
import Foundation
import ObjectiveC.runtime

enum DesktopMaterial: String, Codable {
    case liquidGlass = "liquid_glass", vibrancy, opaque, unavailable
}

struct DesktopDisplayPreferences: Equatable {
    let reduceTransparency: Bool
    let reduceMotion: Bool
    let increaseContrast: Bool

    @MainActor
    static var current: Self {
        let workspace = NSWorkspace.shared
        return Self(reduceTransparency: workspace.accessibilityDisplayShouldReduceTransparency,
                    reduceMotion: workspace.accessibilityDisplayShouldReduceMotion,
                    increaseContrast: workspace.accessibilityDisplayShouldIncreaseContrast)
    }
}

func desktopMaterial(base: DesktopMaterial, preferences: DesktopDisplayPreferences) -> DesktopMaterial {
    guard base != .unavailable else { return .unavailable }
    return preferences.reduceTransparency || preferences.increaseContrast ? .opaque : base
}

struct DesktopEnvironmentSnapshot: Codable, Equatable {
    let material: DesktopMaterial
    let appearance: String
    let reduceTransparency: Bool
    let reduceMotion: Bool
    let increaseContrast: Bool
    let titlebarHeight: Double
    let titlebarControlsWidth: Double
    let focused: Bool

    enum CodingKeys: String, CodingKey {
        case material, appearance, focused
        case reduceTransparency = "reduce_transparency", reduceMotion = "reduce_motion"
        case increaseContrast = "increase_contrast", titlebarHeight = "titlebar_height"
        case titlebarControlsWidth = "titlebar_controls_width"
    }

    var json: String? {
        (try? JSONEncoder().encode(self)).flatMap { String(data: $0, encoding: .utf8) }
    }

    @MainActor
    static func current(view: NSView, material: DesktopMaterial, preferences: DesktopDisplayPreferences) -> Self {
        let window = view.window
        let titlebarHeight: Double
        let titlebarControlsWidth: Double
        if let window, window.styleMask.contains(.titled), !window.styleMask.contains(.fullScreen), let content = window.contentView {
            titlebarHeight = Double(max(0, content.bounds.height - window.contentLayoutRect.height))
            titlebarControlsWidth = Double([NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton]
                .compactMap { window.standardWindowButton($0) }
                .filter { !$0.isHidden }
                .map { content.convert($0.bounds, from: $0).maxX }.max() ?? 0)
        } else { titlebarHeight = 0; titlebarControlsWidth = 0 }
        return Self(material: desktopMaterial(base: material, preferences: preferences),
                    appearance: view.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua ? "dark" : "light",
                    reduceTransparency: preferences.reduceTransparency, reduceMotion: preferences.reduceMotion,
                    increaseContrast: preferences.increaseContrast, titlebarHeight: titlebarHeight,
                    titlebarControlsWidth: titlebarControlsWidth,
                    focused: window?.isKeyWindow == true && NSApp.isActive)
    }
}

public typealias DesktopEnvironmentSink = @convention(c) (UnsafePointer<CChar>?, UnsafePointer<CChar>?) -> Void

@MainActor
final class DesktopMaterialPresentation: NSView {
    let content: NSView

    init(content: NSView) {
        self.content = content
        super.init(frame: NSRect(origin: .zero, size: content.frame.size))
        wantsLayer = true
        content.removeFromSuperview()
        content.frame = bounds
        content.translatesAutoresizingMaskIntoConstraints = true
        content.autoresizingMask = [.width, .height]
        addSubview(content)
    }

    required init?(coder: NSCoder) { nil }
}

@MainActor
private final class DesktopMaterialRoot: NSView {
    var environmentChanged: (() -> Void)?

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        environmentChanged?()
    }
}

private var desktopMaterialHostKey: UInt8 = 0

@MainActor
private final class DesktopMainWindowChrome {
    private weak var window: NSWindow?
    private let previousToolbar: NSToolbar?
    private let previousToolbarVisible: Bool
    private let previousStyle: NSWindow.ToolbarStyle
    private let toolbar: NSToolbar

    init(window: NSWindow) {
        self.window = window
        previousToolbar = window.toolbar
        previousToolbarVisible = window.toolbar?.isVisible ?? false
        previousStyle = window.toolbarStyle
        toolbar = NSToolbar(identifier: "PennelMainWindowGeometry")
        toolbar.displayMode = .iconOnly
        toolbar.allowsUserCustomization = false
        toolbar.autosavesConfiguration = false
        if #available(macOS 15, *) { toolbar.allowsDisplayModeCustomization = false }
        // AppKit supplies titlebar geometry and traffic-light placement; the controls remain in the webview.
        window.toolbarStyle = .unified
        window.toolbar = toolbar
        toolbar.isVisible = true
        window.contentView?.layoutSubtreeIfNeeded()
    }

    func restore() {
        guard let window, window.toolbar === toolbar else { return }
        let restoreStyle = window.toolbarStyle == .unified
        window.toolbar = previousToolbar
        previousToolbar?.isVisible = previousToolbarVisible
        if restoreStyle { window.toolbarStyle = previousStyle }
    }
}

@MainActor
final class DesktopMaterialHost: NSObject {
    let root: NSView
    let materialView: NSView
    let presentation: DesktopMaterialPresentation
    private weak var window: NSWindow?
    private weak var originalParent: NSView?
    private let originalAutoresizing: NSView.AutoresizingMask
    private let originalTranslatesConstraints: Bool
    private let originalConstraints: [NSLayoutConstraint]
    private let mainWindowChrome: DesktopMainWindowChrome?
    private var replacementConstraints: [NSLayoutConstraint] = []
    private let baseMaterial: DesktopMaterial
    private let preferences: () -> DesktopDisplayPreferences
    private var observers: [(NotificationCenter, NSObjectProtocol)] = []
    private var lastPublished: DesktopEnvironmentSnapshot?
    private var label: String
    private var sink: DesktopEnvironmentSink?
    private var detached = false

    static func existing(for view: NSView) -> DesktopMaterialHost? {
        guard let window = view.window,
              let host = objc_getAssociatedObject(window, &desktopMaterialHostKey) as? DesktopMaterialHost,
              host.presentation.content === view, !host.detached else { return nil }
        return host
    }

    static func attach(to content: NSView, floating: Bool, label: String = "", sink: DesktopEnvironmentSink? = nil,
                       preferences: (() -> DesktopDisplayPreferences)? = nil, allowLiquidGlass: Bool = true) -> DesktopMaterialHost? {
        if let existing = existing(for: content) {
            existing.label = label
            existing.sink = sink
            existing.refresh()
            return existing
        }
        guard let window = content.window, let parent = content.superview else { return nil }
        guard objc_getAssociatedObject(window, &desktopMaterialHostKey) == nil else { return nil }
        let host = DesktopMaterialHost(content: content, parent: parent, window: window, floating: floating,
                                       label: label, sink: sink, preferences: preferences ?? { .current }, allowLiquidGlass: allowLiquidGlass)
        objc_setAssociatedObject(window, &desktopMaterialHostKey, host, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        host.observe()
        host.refresh()
        return host
    }

    private init(content: NSView, parent: NSView, window: NSWindow, floating: Bool, label: String,
                 sink: DesktopEnvironmentSink?, preferences: @escaping () -> DesktopDisplayPreferences, allowLiquidGlass: Bool) {
        self.window = window
        self.originalParent = parent
        self.label = label
        self.sink = sink
        self.preferences = preferences
        mainWindowChrome = label == "main" && !floating && window.styleMask.contains(.titled) && !(window is NSPanel)
            ? DesktopMainWindowChrome(window: window) : nil
        originalAutoresizing = content.autoresizingMask
        originalTranslatesConstraints = content.translatesAutoresizingMaskIntoConstraints
        originalConstraints = parent.constraints.filter { ($0.firstItem as? NSView) === content || ($0.secondItem as? NSView) === content }
        NSLayoutConstraint.deactivate(originalConstraints)
        let originalFrame = content.frame
        let siblingAbove = parent.subviews.drop(while: { $0 !== content }).dropFirst().first
        let root = DesktopMaterialRoot(frame: originalFrame)
        self.root = root
        root.autoresizingMask = originalAutoresizing
        root.translatesAutoresizingMaskIntoConstraints = originalTranslatesConstraints
        presentation = DesktopMaterialPresentation(content: content)
        if #available(macOS 26, *), allowLiquidGlass {
            let glass = NSGlassEffectView(frame: root.bounds)
            glass.style = .regular
            glass.cornerRadius = floating ? 12 : 0
            // AppKit guarantees glass composition for contentView, not arbitrary sibling views.
            glass.contentView = presentation
            materialView = glass
            baseMaterial = .liquidGlass
        } else {
            let visual = NSVisualEffectView(frame: root.bounds)
            visual.material = floating ? .popover : .sidebar
            visual.blendingMode = .behindWindow
            visual.state = .followsWindowActiveState
            visual.addSubview(presentation)
            presentation.frame = visual.bounds
            presentation.autoresizingMask = [.width, .height]
            materialView = visual
            baseMaterial = .vibrancy
        }
        super.init()
        materialView.autoresizingMask = [.width, .height]
        root.addSubview(materialView)
        parent.addSubview(root, positioned: siblingAbove == nil ? .above : .below, relativeTo: siblingAbove)
        replacementConstraints = originalConstraints.map { constraint in
            let replacement = NSLayoutConstraint(item: (constraint.firstItem as? NSView) === content ? root : constraint.firstItem!,
                                                 attribute: constraint.firstAttribute, relatedBy: constraint.relation,
                                                 toItem: (constraint.secondItem as? NSView) === content ? root : constraint.secondItem,
                                                 attribute: constraint.secondAttribute, multiplier: constraint.multiplier, constant: constraint.constant)
            replacement.priority = constraint.priority
            replacement.identifier = constraint.identifier
            return replacement
        }
        NSLayoutConstraint.activate(replacementConstraints)
        root.environmentChanged = { [weak self] in self?.refresh() }
    }

    var snapshot: DesktopEnvironmentSnapshot {
        .current(view: root, material: baseMaterial, preferences: preferences())
    }

    func refresh() {
        guard !detached else { return }
        let next = snapshot
        var background = NSColor.clear.cgColor
        if next.material == .opaque {
            root.effectiveAppearance.performAsCurrentDrawingAppearance { background = NSColor.windowBackgroundColor.cgColor }
        }
        presentation.layer?.backgroundColor = background
        guard next != lastPublished else { return }
        lastPublished = next
        if let sink, let json = next.json {
            label.withCString { label in json.withCString { value in sink(label, value) } }
        }
    }

    private func observe() {
        let center = NotificationCenter.default
        let windowNotifications: [Notification.Name] = [NSWindow.didBecomeKeyNotification, NSWindow.didResignKeyNotification, NSWindow.didBecomeMainNotification, NSWindow.didResignMainNotification,
                                                        NSWindow.didResizeNotification, NSWindow.didChangeScreenNotification,
                                                        NSWindow.didChangeBackingPropertiesNotification,
                                                        NSWindow.didEnterFullScreenNotification, NSWindow.didExitFullScreenNotification]
        for name in windowNotifications {
            observers.append((center, center.addObserver(forName: name, object: window, queue: .main) { [weak self] _ in MainActor.assumeIsolated { self?.refresh() } }))
        }
        for name in [NSApplication.didBecomeActiveNotification, NSApplication.didResignActiveNotification] {
            observers.append((center, center.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in MainActor.assumeIsolated { self?.refresh() } }))
        }
        observers.append((center, center.addObserver(forName: NSWindow.willCloseNotification, object: window, queue: .main) { [weak self] _ in MainActor.assumeIsolated { self?.detach() } }))
        let workspace = NSWorkspace.shared.notificationCenter
        observers.append((workspace, workspace.addObserver(forName: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification, object: nil, queue: .main) { [weak self] _ in MainActor.assumeIsolated { self?.refresh() } }))
    }

    func detach() {
        guard !detached else { return }
        detached = true
        for (center, observer) in observers { center.removeObserver(observer) }
        observers.removeAll()
        (root as? DesktopMaterialRoot)?.environmentChanged = nil
        sink = nil
        NSLayoutConstraint.deactivate(replacementConstraints)
        let content = presentation.content
        content.removeFromSuperview()
        content.translatesAutoresizingMaskIntoConstraints = originalTranslatesConstraints
        content.autoresizingMask = originalAutoresizing
        content.frame = root.frame
        if let parent = originalParent { parent.addSubview(content, positioned: .below, relativeTo: root.superview === parent ? root : nil) }
        root.removeFromSuperview()
        NSLayoutConstraint.activate(originalConstraints)
        mainWindowChrome?.restore()
        if let window, objc_getAssociatedObject(window, &desktopMaterialHostKey) as? DesktopMaterialHost === self {
            objc_setAssociatedObject(window, &desktopMaterialHostKey, nil, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        }
    }

    deinit {
        for (center, observer) in observers { center.removeObserver(observer) }
    }
}

@_cdecl("pennel_mac_desktop_material_attach")
public func desktopMaterialAttach(_ pointer: UnsafeMutableRawPointer?, _ rawLabel: UnsafePointer<CChar>?, _ floating: Bool, _ sink: DesktopEnvironmentSink?) -> UnsafeMutablePointer<CChar>? {
    guard let pointer, let label = decodeString(rawLabel) else { return nil }
    let json = onMain {
        MainActor.assumeIsolated {
            let view = Unmanaged<NSView>.fromOpaque(pointer).takeUnretainedValue()
            return DesktopMaterialHost.attach(to: view, floating: floating, label: label, sink: sink)?.snapshot.json
        }
    }
    return json.flatMap(bridgeString)
}

@_cdecl("pennel_mac_desktop_environment")
public func desktopEnvironmentJSON(_ pointer: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>? {
    guard let pointer else { return nil }
    let json: String? = onMain {
        MainActor.assumeIsolated { () -> String? in
            let view = Unmanaged<NSView>.fromOpaque(pointer).takeUnretainedValue()
            if let host = DesktopMaterialHost.existing(for: view) {
                host.refresh()
                return host.snapshot.json
            }
            return DesktopEnvironmentSnapshot.current(view: view, material: .unavailable, preferences: .current).json
        }
    }
    return json.flatMap(bridgeString)
}

@_cdecl("pennel_mac_desktop_material_detach")
public func desktopMaterialDetach(_ pointer: UnsafeMutableRawPointer?) {
    guard let pointer else { return }
    onMain { MainActor.assumeIsolated { DesktopMaterialHost.existing(for: Unmanaged<NSView>.fromOpaque(pointer).takeUnretainedValue())?.detach() } }
}

@_cdecl("pennel_mac_desktop_material_appearance")
public func desktopMaterialAppearance(_ pointer: UnsafeMutableRawPointer?, _ appearance: Int32) {
    guard let pointer else { return }
    onMain {
        MainActor.assumeIsolated {
            let view = Unmanaged<NSView>.fromOpaque(pointer).takeUnretainedValue()
            // A nonactivating panel owns the presentation after Tauri's window has been hidden.
            view.window?.appearance = appearance == 1 ? NSAppearance(named: .aqua) : appearance == 2 ? NSAppearance(named: .darkAqua) : nil
            DesktopMaterialHost.existing(for: view)?.refresh()
        }
    }
}
