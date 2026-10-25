import AppKit
import PennelKit
import SwiftUI

@MainActor
final class PillPanel {
    let model = PillModel()
    private let panel: NSPanel
    private var anchorScreenNumber: NSNumber?
    private var layoutTask: Task<Void, Never>?

    init() {
        panel = NonActivatingPanel(
            contentRect: NSRect(x: 0, y: 0, width: ThemePill.windowWidth, height: ThemePill.windowHeight),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: true
        )
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.level = .statusBar
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        panel.ignoresMouseEvents = true
        panel.contentView = NSHostingView(rootView: PillView(model: model))
    }

    func show(_ state: PillState) {
        guard state != model.state || state != .hidden else { return }
        let wasHidden = model.state == .hidden
        model.show(state)
        if case .hidden = state {
            layoutTask?.cancel()
            anchorScreenNumber = nil
            panel.orderOut(nil)
            return
        }
        if wasHidden { anchorScreenNumber = nil }
        position()
        panel.orderFrontRegardless()
        scheduleLayout()
    }

    func beginPreview(_ sessionID: String) {
        model.preview.begin(sessionID)
        scheduleLayout()
    }

    func updatePreview(sessionID: String, revision: UInt64, text: String) {
        guard model.preview.update(sessionID: sessionID, revision: revision, text: text) else { return }
        scheduleLayout()
    }

    func clearPreview(sessionID: String? = nil) {
        model.preview.clear(sessionID: sessionID)
        scheduleLayout()
    }

    private func scheduleLayout() {
        layoutTask?.cancel()
        guard model.state != .hidden else { return }
        layoutTask = Task { @MainActor [weak self] in
            await Task.yield()
            guard !Task.isCancelled, let self, self.model.state != .hidden else { return }
            self.position()
        }
    }

    private func position() {
        guard let content = panel.contentView else { return }
        let mouse = NSEvent.mouseLocation
        let screen = NSScreen.screens.first { $0.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber == anchorScreenNumber }
            ?? NSScreen.screens.first { NSMouseInRect(mouse, $0.frame, false) } ?? NSScreen.main
        guard let frame = screen?.visibleFrame else { return }
        anchorScreenNumber = screen?.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber
        let maximumWidth = min(ThemePill.windowWidth, frame.width)
        model.contentWidth = max(0, maximumWidth - 2 * ThemePill.outerInset)
        content.invalidateIntrinsicContentSize()
        content.layoutSubtreeIfNeeded()
        let fitting = content.fittingSize
        let size = NSSize(width: min(maximumWidth, fitting.width), height: min(ThemePill.windowHeight, frame.height, fitting.height))
        panel.setFrame(
            NSRect(x: frame.midX - size.width / 2, y: frame.minY, width: size.width, height: size.height),
            display: true
        )
    }
}

private final class NonActivatingPanel: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}
