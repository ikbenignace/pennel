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
    func testNativeEnvironmentCallbacksAreDeferredAndCoalesceTheLatestSnapshot() throws {
        try withWindow { _, _, webview in
            receivedDesktopEnvironments = []
            var preferences = DesktopDisplayPreferences(reduceTransparency: false, reduceMotion: false, increaseContrast: false)
            let sink: DesktopEnvironmentSink = { label, json in
                guard let label, let json else { return }
                let event = String(cString: label) + ":" + String(cString: json)
                MainActor.assumeIsolated { receivedDesktopEnvironments.append(event) }
            }
            let host = try XCTUnwrap(DesktopMaterialHost.attach(to: webview, floating: false, label: "main", sink: sink, preferences: { preferences }))
            defer { host.detach() }
            XCTAssertTrue(receivedDesktopEnvironments.isEmpty)
            preferences = .init(reduceTransparency: false, reduceMotion: true, increaseContrast: false)
            host.refresh()
            preferences = .init(reduceTransparency: true, reduceMotion: true, increaseContrast: false)
            host.refresh()
            XCTAssertTrue(receivedDesktopEnvironments.isEmpty)
            drainDesktopEnvironmentEvents()
            XCTAssertEqual(receivedDesktopEnvironments.count, 1)
            XCTAssertTrue(receivedDesktopEnvironments.last?.contains("\"material\":\"opaque\"") == true)
            XCTAssertTrue(receivedDesktopEnvironments.last?.contains("\"reduce_motion\":true") == true)
            preferences = .init(reduceTransparency: false, reduceMotion: false, increaseContrast: false)
            host.refresh()
            XCTAssertEqual(receivedDesktopEnvironments.count, 1)
            drainDesktopEnvironmentEvents()
            XCTAssertEqual(receivedDesktopEnvironments.count, 2)
            XCTAssertTrue(receivedDesktopEnvironments.last?.contains("\"reduce_transparency\":false") == true)
            host.refresh()
            drainDesktopEnvironmentEvents()
            XCTAssertEqual(receivedDesktopEnvironments.count, 2)
            preferences = .init(reduceTransparency: true, reduceMotion: false, increaseContrast: false)
            host.refresh()
            preferences = .init(reduceTransparency: false, reduceMotion: false, increaseContrast: false)
            host.refresh()
            drainDesktopEnvironmentEvents()
            XCTAssertEqual(receivedDesktopEnvironments.count, 2)
        }
    }

    @MainActor
    func testQueuedCallbacksUseTheCurrentLabelAndSinkAfterRepeatedAttachment() throws {
        try withWindow { _, _, webview in
            receivedDesktopEnvironments = []
            let oldSink: DesktopEnvironmentSink = { _, _ in
                MainActor.assumeIsolated { receivedDesktopEnvironments.append("stale sink") }
            }
            let currentSink: DesktopEnvironmentSink = { label, json in
                guard let label, let json else { return }
                let event = String(cString: label) + ":" + String(cString: json)
                MainActor.assumeIsolated { receivedDesktopEnvironments.append(event) }
            }
            let host = try XCTUnwrap(DesktopMaterialHost.attach(to: webview, floating: false, label: "old", sink: oldSink))
            defer { host.detach() }
            XCTAssertTrue(DesktopMaterialHost.attach(to: webview, floating: false, label: "current", sink: currentSink) === host)
            XCTAssertTrue(receivedDesktopEnvironments.isEmpty)
            drainDesktopEnvironmentEvents()
            XCTAssertEqual(receivedDesktopEnvironments.count, 1)
            XCTAssertTrue(receivedDesktopEnvironments.first?.hasPrefix("current:") == true)
        }
    }

    @MainActor
    func testDetachedHostsDropQueuedCallbacksBeforeAReplacementAttaches() throws {
        try withWindow { _, _, webview in
            receivedDesktopEnvironments = []
            let sink: DesktopEnvironmentSink = { label, json in
                guard let label, let json else { return }
                let event = String(cString: label) + ":" + String(cString: json)
                MainActor.assumeIsolated { receivedDesktopEnvironments.append(event) }
            }
            let old = try XCTUnwrap(DesktopMaterialHost.attach(to: webview, floating: false, label: "old", sink: sink))
            old.detach()
            let replacement = try XCTUnwrap(DesktopMaterialHost.attach(to: webview, floating: false, label: "replacement", sink: sink))
            defer { replacement.detach() }
            XCTAssertTrue(receivedDesktopEnvironments.isEmpty)
            drainDesktopEnvironmentEvents()
            XCTAssertEqual(receivedDesktopEnvironments.count, 1)
            XCTAssertTrue(receivedDesktopEnvironments.first?.hasPrefix("replacement:") == true)
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
            XCTAssertTrue(receivedDesktopEnvironments.isEmpty)
            drainDesktopEnvironmentEvents()
            XCTAssertEqual(receivedDesktopEnvironments.count, 1)
            preferences = .init(reduceTransparency: true, reduceMotion: true, increaseContrast: false)
            NSWorkspace.shared.notificationCenter.post(name: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification, object: nil)
            XCTAssertEqual(receivedDesktopEnvironments.count, 1)
            drainDesktopEnvironmentEvents()
            XCTAssertEqual(receivedDesktopEnvironments.count, 2)
            XCTAssertTrue(receivedDesktopEnvironments.last?.contains("main:") == true)
            XCTAssertTrue(receivedDesktopEnvironments.last?.contains("\"material\":\"opaque\"") == true)
            NSWorkspace.shared.notificationCenter.post(name: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification, object: nil)
            drainDesktopEnvironmentEvents()
            XCTAssertEqual(receivedDesktopEnvironments.count, 2)
            host.detach()
            preferences = .init(reduceTransparency: false, reduceMotion: false, increaseContrast: false)
            NSWorkspace.shared.notificationCenter.post(name: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification, object: nil)
            drainDesktopEnvironmentEvents()
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
            for (index, edge) in edges.enumerated() {
                edge.identifier = "webview-edge-\(index)"
                edge.priority = NSLayoutConstraint.Priority(rawValue: 999 - Float(index))
            }
            NSLayoutConstraint.activate(edges)
            parent.layoutSubtreeIfNeeded()
            let host = try XCTUnwrap(DesktopMaterialHost.attach(to: webview, floating: false))
            XCTAssertTrue(parent.subviews[0] === host.root)
            XCTAssertTrue(parent.subviews[1] === sibling)
            XCTAssertTrue(edges.allSatisfy { !$0.isActive })
            for edge in edges {
                let replacement = try XCTUnwrap(parent.constraints.first { $0.identifier == edge.identifier })
                XCTAssertEqual(replacement.firstAttribute, edge.firstAttribute)
                XCTAssertEqual(replacement.secondAttribute, edge.secondAttribute)
                XCTAssertEqual(replacement.relation, edge.relation)
                XCTAssertEqual(replacement.multiplier, edge.multiplier)
                XCTAssertEqual(replacement.constant, edge.constant)
                XCTAssertEqual(replacement.priority, edge.priority)
            }
            parent.layoutSubtreeIfNeeded()
            XCTAssertEqual(host.root.frame.size, parent.bounds.size)
            host.detach()
            parent.layoutSubtreeIfNeeded()
            XCTAssertTrue(parent.subviews[0] === webview)
            XCTAssertTrue(parent.subviews[1] === sibling)
            XCTAssertFalse(webview.translatesAutoresizingMaskIntoConstraints)
            XCTAssertTrue(edges.allSatisfy(\.isActive))
            XCTAssertEqual(webview.frame.size, parent.bounds.size)
        }
    }

    @MainActor
    func testLaidOutAutoresizingConstraintsSurviveMaterialAttachmentAndTeardown() throws {
        for allowLiquidGlass in [true, false] {
            try withWindow { window, parent, webview in
                window.orderFront(nil)
                parent.updateConstraintsForSubtreeIfNeeded()
                parent.layoutSubtreeIfNeeded()
                window.displayIfNeeded()
                let generated = parent.constraints.filter { ($0.firstItem as? NSView) === webview || ($0.secondItem as? NSView) === webview }
                XCTAssertTrue(generated.contains { $0.secondItem == nil && ![NSLayoutConstraint.Attribute.width, .height].contains($0.firstAttribute) })
                let host = try XCTUnwrap(DesktopMaterialHost.attach(to: webview, floating: false, label: "main", allowLiquidGlass: allowLiquidGlass))
                XCTAssertTrue(window.contentView === parent)
                XCTAssertTrue(webview.window === window)
                window.setContentSize(NSSize(width: 810, height: 590))
                parent.layoutSubtreeIfNeeded()
                host.root.layoutSubtreeIfNeeded()
                XCTAssertEqual(host.root.frame.size, parent.bounds.size)
                XCTAssertEqual(webview.bounds.size, host.presentation.bounds.size)
                host.detach()
                parent.layoutSubtreeIfNeeded()
                XCTAssertTrue(webview.superview === parent)
                XCTAssertTrue(webview.translatesAutoresizingMaskIntoConstraints)
                XCTAssertEqual(webview.autoresizingMask, [.width, .height])
                XCTAssertEqual(webview.frame.size, parent.bounds.size)
            }
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
    func testMainHostUsesAnEmptySystemToolbarAndReportsItsNativeGeometry() throws {
        try withWindow { window, parent, webview in
            window.titlebarAppearsTransparent = true
            let host = try XCTUnwrap(DesktopMaterialHost.attach(to: webview, floating: false, label: "main"))
            defer { host.detach() }
            let toolbar = try XCTUnwrap(window.toolbar)
            XCTAssertEqual(window.toolbarStyle, .unified)
            XCTAssertEqual(toolbar.displayMode, .iconOnly)
            XCTAssertTrue(toolbar.isVisible)
            XCTAssertFalse(toolbar.allowsUserCustomization)
            XCTAssertFalse(toolbar.autosavesConfiguration)
            if #available(macOS 15, *) { XCTAssertFalse(toolbar.allowsDisplayModeCustomization) }
            XCTAssertTrue(toolbar.items.isEmpty)
            XCTAssertTrue(window.titlebarAppearsTransparent)
            XCTAssertTrue(window.styleMask.contains(.fullSizeContentView))
            XCTAssertTrue(window.contentView === parent)
            XCTAssertTrue(webview.window === window)
            let buttons = [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton]
                .compactMap { window.standardWindowButton($0) }
            let frames = buttons.map { parent.convert($0.bounds, from: $0) }
            XCTAssertEqual(frames.count, 3)
            XCTAssertGreaterThan(host.snapshot.titlebarHeight, 40)
            for frame in frames {
                XCTAssertEqual(Double(parent.bounds.height - frame.midY), host.snapshot.titlebarHeight / 2, accuracy: 1)
            }
            XCTAssertEqual(host.snapshot.titlebarControlsWidth, Double(try XCTUnwrap(frames.map(\.maxX).max())), accuracy: 1)
            XCTAssertTrue(DesktopMaterialHost.attach(to: webview, floating: false, label: "main") === host)
            XCTAssertTrue(window.toolbar === toolbar)
            XCTAssertEqual(buttons.map { parent.convert($0.bounds, from: $0) }, frames)
        }
    }

    @MainActor
    func testMainChromeRestoresThePreviousToolbarAndStyleAtTeardown() throws {
        try withWindow { window, parent, webview in
            let previous = NSToolbar(identifier: "PreviousToolbar")
            previous.autosavesConfiguration = false
            previous.displayMode = .labelOnly
            window.toolbarStyle = .expanded
            window.toolbar = previous
            previous.isVisible = false
            let host = try XCTUnwrap(DesktopMaterialHost.attach(to: webview, floating: false, label: "main", allowLiquidGlass: false))
            XCTAssertFalse(window.toolbar === previous)
            XCTAssertEqual(window.toolbarStyle, .unified)
            XCTAssertTrue(host.materialView is NSVisualEffectView)
            host.detach()
            XCTAssertTrue(window.toolbar === previous)
            XCTAssertEqual(window.toolbarStyle, .expanded)
            XCTAssertEqual(previous.displayMode, .labelOnly)
            XCTAssertFalse(previous.isVisible)
            XCTAssertTrue(webview.superview === parent)
        }
    }

    @MainActor
    func testChromeTeardownPreservesAToolbarInstalledByAnotherOwner() throws {
        try withWindow { window, _, webview in
            let host = try XCTUnwrap(DesktopMaterialHost.attach(to: webview, floating: false, label: "main"))
            let replacement = NSToolbar(identifier: "ReplacementToolbar")
            window.toolbar = replacement
            window.toolbarStyle = .expanded
            host.detach()
            XCTAssertTrue(window.toolbar === replacement)
            XCTAssertEqual(window.toolbarStyle, .expanded)
        }
    }

    @MainActor
    func testOtherWindowLabelsAndFloatingHostsKeepTheirOriginalChrome() throws {
        for (label, floating) in [("onboarding", false), ("", false), ("main", true)] {
            try withWindow { window, _, webview in
                let style = window.toolbarStyle
                let host = try XCTUnwrap(DesktopMaterialHost.attach(to: webview, floating: floating, label: label))
                defer { host.detach() }
                XCTAssertNil(window.toolbar)
                XCTAssertEqual(window.toolbarStyle, style)
            }
        }
    }

    @MainActor
    private func drainDesktopEnvironmentEvents() {
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.02))
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
