import AppKit
import SwiftUI

@MainActor
final class PillPanel {
    let model = PillModel()
    private let panel: NSPanel
    private var hideTask: Task<Void, Never>?

    init() {
        panel = NonActivatingPanel(
            contentRect: NSRect(x: 0, y: 0, width: 320, height: 72),
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
        hideTask?.cancel()
        model.state = state
        if case .hidden = state {
            panel.orderOut(nil)
            return
        }
        position()
        panel.orderFrontRegardless()
        if case .error = state {
            hideTask = Task { [weak self] in
                try? await Task.sleep(for: .seconds(2))
                guard !Task.isCancelled else { return }
                self?.show(.hidden)
            }
        }
    }

    private func position() {
        guard let content = panel.contentView else { return }
        let size = content.fittingSize
        let mouse = NSEvent.mouseLocation
        let screen = NSScreen.screens.first { NSMouseInRect(mouse, $0.frame, false) } ?? NSScreen.main
        guard let frame = screen?.visibleFrame else { return }
        panel.setFrame(
            NSRect(x: frame.midX - size.width / 2, y: frame.minY + 2, width: size.width, height: size.height),
            display: true
        )
    }
}

private final class NonActivatingPanel: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}
