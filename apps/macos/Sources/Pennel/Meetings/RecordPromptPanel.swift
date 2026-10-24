import AppKit
import PennelKit
import SwiftUI

enum PromptAnswer {
    case yes, no, never
}

/// Small "Opnemen?" card in the top-right corner that never steals focus from the call.
@MainActor
final class RecordPromptPanel {
    private var panel: NSPanel?
    private var timeout: Task<Void, Never>?

    func show(appName: String, eventTitle: String?, onAnswer: @escaping (PromptAnswer) -> Void) {
        dismiss()
        let panel = PromptPanel(
            contentRect: NSRect(x: 0, y: 0, width: 340, height: 120),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: true
        )
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.level = .statusBar
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.appearance = NSAppearance(named: .aqua)
        let answer: (PromptAnswer) -> Void = { [weak self] choice in
            self?.dismiss()
            onAnswer(choice)
        }
        panel.contentView = NSHostingView(rootView: PromptView(appName: appName, eventTitle: eventTitle, onAnswer: answer))
        if let screen = NSScreen.main?.visibleFrame, let size = panel.contentView?.fittingSize {
            panel.setFrame(NSRect(x: screen.maxX - size.width - 12, y: screen.maxY - size.height - 12, width: size.width, height: size.height), display: true)
        }
        panel.orderFrontRegardless()
        self.panel = panel
        timeout = Task { [weak self] in
            try? await Task.sleep(for: .seconds(60))
            guard !Task.isCancelled else { return }
            self?.dismiss()
            onAnswer(.no)
        }
    }

    func dismiss() {
        timeout?.cancel()
        timeout = nil
        panel?.orderOut(nil)
        panel = nil
    }

    var isShowing: Bool { panel != nil }
}

private final class PromptPanel: NSPanel {
    override var canBecomeKey: Bool { false }
}

private struct PromptView: View {
    let appName: String
    let eventTitle: String?
    let onAnswer: (PromptAnswer) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 8) {
                Circle().fill(Theme.accent).frame(width: 7, height: 7)
                Text("Opnemen?").font(.figtree(14, weight: .semibold))
            }
            Text([appName, eventTitle].compactMap { $0 }.joined(separator: " · "))
                .font(.figtree(12))
                .foregroundStyle(Theme.muted)
                .lineLimit(2)
            HStack(spacing: 8) {
                Button("Ja") { onAnswer(.yes) }.keyboardShortcut(.defaultAction)
                Button("Nee") { onAnswer(.no) }
                Spacer()
                Button("Nooit voor \(appName)") { onAnswer(.never) }
                    .buttonStyle(.plain)
                    .font(.figtree(11))
                    .foregroundStyle(Theme.muted)
            }
        }
        .padding(16)
        .frame(width: 320)
        .background(RoundedRectangle(cornerRadius: 14).fill(Theme.paper))
        .overlay(RoundedRectangle(cornerRadius: 14).stroke(Theme.line))
        .shadow(color: .black.opacity(0.18), radius: 16, y: 6)
        .padding(10)
        .foregroundStyle(Theme.ink)
    }
}
