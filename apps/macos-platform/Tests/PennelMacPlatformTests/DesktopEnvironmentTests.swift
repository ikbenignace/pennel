import AppKit
import ObjectiveC.runtime
import WebKit
import XCTest
@testable import PennelMacPlatform

@MainActor
private var receivedDesktopEnvironments: [String] = []

final class DesktopEnvironmentTests: XCTestCase {
    func testOpaquePolicyOverridesBothNativeMaterials() {
        let normal = DesktopDisplayPreferences(reduceTransparency: false, reduceMotion: false, increaseContrast: false)
        XCTAssertEqual(desktopMaterial(base: .liquidGlass, preferences: normal), .liquidGlass)
        XCTAssertEqual(desktopMaterial(base: .vibrancy, preferences: normal), .vibrancy)
        XCTAssertEqual(desktopMaterial(base: .liquidGlass, preferences: .init(reduceTransparency: true, reduceMotion: false, increaseContrast: false)), .opaque)
        XCTAssertEqual(desktopMaterial(base: .vibrancy, preferences: .init(reduceTransparency: false, reduceMotion: false, increaseContrast: true)), .opaque)
        XCTAssertEqual(desktopMaterial(base: .unavailable, preferences: normal), .unavailable)
    }

    @MainActor
    func testGlassEmbedsTheExistingWebviewAndRepeatedAttachmentPreservesIdentity() throws {
        try withWindow { window, parent, webview in
            let originalClass: AnyClass? = object_getClass(window)
            let host = try XCTUnwrap(DesktopMaterialHost.attach(to: webview, floating: false))
            defer { host.detach() }
            XCTAssertTrue(window.contentView === parent)
            XCTAssertTrue(object_getClass(window) === originalClass)
            XCTAssertTrue(host.presentation.content === webview)
            XCTAssertTrue(webview.window === window)
            XCTAssertTrue(DesktopMaterialHost.attach(to: webview, floating: false) === host)
            XCTAssertEqual(parent.subviews.count, 1)
            if #available(macOS 26, *) {
                let glass = try XCTUnwrap(host.materialView as? NSGlassEffectView)
                XCTAssertTrue(glass.contentView === host.presentation)
                XCTAssertEqual(glass.style, .regular)
            }
        }
    }

    @MainActor
    func testAccessibilityChangesKeepTheEditorAndNativeHierarchyInPlace() throws {
        try withWindow { _, _, webview in
            var preferences = DesktopDisplayPreferences(reduceTransparency: false, reduceMotion: false, increaseContrast: false)
            let host = try XCTUnwrap(DesktopMaterialHost.attach(to: webview, floating: false, preferences: { preferences }))
            defer { host.detach() }
            let material = host.materialView
            let presentation = webview.superview
            preferences = .init(reduceTransparency: true, reduceMotion: true, increaseContrast: false)
            host.refresh()
            XCTAssertEqual(host.snapshot.material, .opaque)
            XCTAssertTrue(host.snapshot.reduceTransparency)
            XCTAssertTrue(host.snapshot.reduceMotion)
            XCTAssertEqual(host.presentation.layer?.backgroundColor?.alpha, 1)
            XCTAssertTrue(host.materialView === material)
            XCTAssertTrue(webview.superview === presentation)
            preferences = .init(reduceTransparency: false, reduceMotion: false, increaseContrast: true)
            host.refresh()
            XCTAssertTrue(host.snapshot.increaseContrast)
            XCTAssertEqual(host.snapshot.material, .opaque)
            preferences = .init(reduceTransparency: false, reduceMotion: false, increaseContrast: false)
            host.refresh()
            XCTAssertNotEqual(host.snapshot.material, .opaque)
            XCTAssertEqual(host.presentation.layer?.backgroundColor?.alpha, 0)
        }
    }

    @MainActor
    func testLegacyFallbackUsesWindowStateAndTeardownRestoresTheWebview() throws {
        try withWindow { window, parent, webview in
            let host = try XCTUnwrap(DesktopMaterialHost.attach(to: webview, floating: false, allowLiquidGlass: false))
            let visual = try XCTUnwrap(host.materialView as? NSVisualEffectView)
            XCTAssertEqual(visual.material, .sidebar)
            XCTAssertEqual(visual.state, .followsWindowActiveState)
            window.setContentSize(NSSize(width: 810, height: 590))
            parent.layoutSubtreeIfNeeded()
            host.root.layoutSubtreeIfNeeded()
            XCTAssertEqual(host.root.frame.size, parent.bounds.size)
            XCTAssertEqual(webview.bounds.size, host.presentation.bounds.size)
            host.detach()
            XCTAssertTrue(webview.superview === parent)
            XCTAssertEqual(parent.subviews.count, 1)
            XCTAssertTrue(parent.subviews.first === webview)
            XCTAssertNil(DesktopMaterialHost.existing(for: webview))
            host.detach()
        }
    }

    @MainActor
    func testPromptUsesTheActualNonactivatingPanelAndNoTitlebarInset() throws {
        try withWindow { window, parent, webview in
            configurePanel(Unmanaged.passUnretained(window).toOpaque(), true)
            let panel = try XCTUnwrap(webview.window as? NSPanel)
            XCTAssertTrue(panel.contentView === parent)
            let host = try XCTUnwrap(DesktopMaterialHost.attach(to: webview, floating: true, allowLiquidGlass: false))
            defer { host.detach() }
            let effect = try XCTUnwrap(host.materialView as? NSVisualEffectView)
            XCTAssertEqual(effect.material, .popover)
            XCTAssertFalse(panel.canBecomeKey)
            XCTAssertFalse(panel.canBecomeMain)
            XCTAssertFalse(panel.ignoresMouseEvents)
            XCTAssertEqual(host.snapshot.titlebarHeight, 0)
            XCTAssertTrue(webview.window === panel)
            desktopMaterialAppearance(Unmanaged.passUnretained(webview).toOpaque(), 2)
            XCTAssertEqual(host.snapshot.appearance, "dark")
            XCTAssertEqual(panel.appearance?.name, .darkAqua)
            desktopMaterialAppearance(Unmanaged.passUnretained(webview).toOpaque(), 1)
            XCTAssertEqual(host.snapshot.appearance, "light")
            desktopMaterialAppearance(Unmanaged.passUnretained(webview).toOpaque(), 0)
            XCTAssertNil(panel.appearance)
        }
    }

    @MainActor
    func testSnapshotEncodingMatchesTheFrontendContractAndAppearanceChangesLive() throws {
        try withWindow { window, _, webview in
            let host = try XCTUnwrap(DesktopMaterialHost.attach(to: webview, floating: false))
            defer { host.detach() }
            window.appearance = NSAppearance(named: .darkAqua)
            host.refresh()
            XCTAssertEqual(host.snapshot.appearance, "dark")
            window.appearance = NSAppearance(named: .aqua)
            host.refresh()
            XCTAssertEqual(host.snapshot.appearance, "light")
            XCTAssertGreaterThan(host.snapshot.titlebarHeight, 0)
            let data = try JSONEncoder().encode(host.snapshot)
            let object = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
            XCTAssertEqual(Set(object.keys), Set(["material", "appearance", "reduce_transparency", "reduce_motion", "increase_contrast", "titlebar_height", "titlebar_controls_width", "focused"]))
        }
    }

    @MainActor
    func testNativeAccessibilityNotificationPublishesOnceAndTeardownRemovesItsObserver() throws {
        try withWindow { _, _, webview in
            receivedDesktopEnvironments = []
            var preferences = DesktopDisplayPreferences(reduceTransparency: false, reduceMotion: false, increaseContrast: false)
            let sink: DesktopEnvironmentSink = { label, json in
                guard let label, let json else { return }
                let event = String(cString: label) + ":" + String(cString: json)
                MainActor.assumeIsolated { receivedDesktopEnvironments.append(event) }
            }
            let host = try XCTUnwrap(DesktopMaterialHost.attach(to: webview, floating: false, label: "main", sink: sink, preferences: { preferences }))
            XCTAssertEqual(receivedDesktopEnvironments.count, 1)
            preferences = .init(reduceTransparency: true, reduceMotion: true, increaseContrast: false)
            NSWorkspace.shared.notificationCenter.post(name: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification, object: nil)
            XCTAssertEqual(receivedDesktopEnvironments.count, 2)
            XCTAssertTrue(receivedDesktopEnvironments.last?.contains("main:") == true)
            XCTAssertTrue(receivedDesktopEnvironments.last?.contains("\"material\":\"opaque\"") == true)
            NSWorkspace.shared.notificationCenter.post(name: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification, object: nil)
            XCTAssertEqual(receivedDesktopEnvironments.count, 2)
            host.detach()
            preferences = .init(reduceTransparency: false, reduceMotion: false, increaseContrast: false)
            NSWorkspace.shared.notificationCenter.post(name: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification, object: nil)
            XCTAssertEqual(receivedDesktopEnvironments.count, 2)
        }
    }

    @MainActor
    func testConstraintsAndSiblingOrderSurviveAttachmentAndTeardown() throws {
        try withWindow { _, parent, webview in
            let sibling = NSView(frame: parent.bounds)
            parent.addSubview(sibling)
            webview.translatesAutoresizingMaskIntoConstraints = false
            let edges = [webview.leadingAnchor.constraint(equalTo: parent.leadingAnchor), webview.trailingAnchor.constraint(equalTo: parent.trailingAnchor),
                         webview.topAnchor.constraint(equalTo: parent.topAnchor), webview.bottomAnchor.constraint(equalTo: parent.bottomAnchor)]
            NSLayoutConstraint.activate(edges)
            let host = try XCTUnwrap(DesktopMaterialHost.attach(to: webview, floating: false))
            XCTAssertTrue(parent.subviews[0] === host.root)
            XCTAssertTrue(parent.subviews[1] === sibling)
            XCTAssertTrue(edges.allSatisfy { !$0.isActive })
            host.detach()
            XCTAssertTrue(parent.subviews[0] === webview)
            XCTAssertTrue(parent.subviews[1] === sibling)
            XCTAssertFalse(webview.translatesAutoresizingMaskIntoConstraints)
            XCTAssertTrue(edges.allSatisfy(\.isActive))
        }
    }

    @MainActor
    func testUnattachedReadStillReportsNativeAppearanceGeometryAndPreferences() throws {
        try withWindow { window, _, webview in
            window.appearance = NSAppearance(named: .darkAqua)
            let pointer = try XCTUnwrap(desktopEnvironmentJSON(Unmanaged.passUnretained(webview).toOpaque()))
            defer { stringFree(pointer) }
            let snapshot = try JSONDecoder().decode(DesktopEnvironmentSnapshot.self, from: Data(String(cString: pointer).utf8))
            XCTAssertEqual(snapshot.material, .unavailable)
            XCTAssertEqual(snapshot.appearance, "dark")
            XCTAssertGreaterThan(snapshot.titlebarHeight, 0)
            XCTAssertGreaterThan(snapshot.titlebarControlsWidth, 0)
            XCTAssertEqual(snapshot.reduceTransparency, NSWorkspace.shared.accessibilityDisplayShouldReduceTransparency)
            XCTAssertEqual(snapshot.reduceMotion, NSWorkspace.shared.accessibilityDisplayShouldReduceMotion)
            XCTAssertEqual(snapshot.increaseContrast, NSWorkspace.shared.accessibilityDisplayShouldIncreaseContrast)
            XCTAssertNil(DesktopMaterialHost.existing(for: webview))
        }
    }

    @MainActor
    func testMaterialHostReservesTheNativeControlsWithoutMovingThem() throws {
        try withWindow { window, parent, webview in
            let buttons = [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton]
                .compactMap { window.standardWindowButton($0) }
            let frames = buttons.map(\.frame)
            let host = try XCTUnwrap(DesktopMaterialHost.attach(to: webview, floating: false))
            defer { host.detach() }
            XCTAssertEqual(buttons.map(\.frame), frames)
            for button in buttons where !button.isHidden {
                XCTAssertGreaterThanOrEqual(host.snapshot.titlebarControlsWidth, Double(parent.convert(button.bounds, from: button).maxX))
            }
        }
    }

    @MainActor
    private func withWindow(_ work: (NSWindow, NSView, WKWebView) throws -> Void) throws {
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: -20_000, y: -20_000, width: 600, height: 440), styleMask: [.titled, .resizable, .fullSizeContentView], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        let parent = try XCTUnwrap(window.contentView)
        let webview = WKWebView(frame: parent.bounds)
        webview.autoresizingMask = [.width, .height]
        parent.addSubview(webview)
        try work(window, parent, webview)
    }
}
