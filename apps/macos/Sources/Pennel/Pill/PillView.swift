import AppKit
import PennelKit
import Observation
import SwiftUI

@MainActor
@Observable
final class PillModel {
    var state: PillState = .hidden
    var level: Float = 0
    var preview = PillPreviewState()
    var contentWidth = ThemePill.windowWidth - 2 * ThemePill.outerInset

    func show(_ state: PillState) {
        self.state = state
        if state == .hidden { preview.clear() }
    }
}

struct PillView: View {
    let model: PillModel

    var body: some View {
        VStack(alignment: .leading, spacing: ThemeSpacing.sm) {
            if !model.preview.text.isEmpty {
                CaptionTail(text: model.preview.text)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .accessibilityLabel("Voorlopige tekst: \(model.preview.text)")
            }
            status
        }
        .padding(.horizontal, ThemeSpacing.md)
        .padding(.vertical, model.preview.text.isEmpty ? ThemeSpacing.sm : ThemeSpacing.md)
        .frame(width: model.preview.text.isEmpty ? nil : model.contentWidth)
        .frame(maxWidth: model.contentWidth)
        .fixedSize(horizontal: model.preview.text.isEmpty, vertical: true)
        .background(RoundedRectangle(cornerRadius: model.preview.text.isEmpty ? ThemeRadius.pill : ThemeRadius.panel).fill(Theme.pillBackground))
        .shadow(color: Theme.shadow, radius: ThemePill.shadowBlur, y: ThemePill.shadowY)
        .padding(ThemePill.outerInset)
    }

    private var status: some View {
        HStack(spacing: ThemeSpacing.sm) {
            switch model.state {
            case .listening, .handsFree, .command:
                Wave(level: model.level)
            case .working:
                WorkIndicator()
            case .error:
                Circle().fill(Theme.pillAccent).frame(width: 7, height: 7)
            case .hidden:
                EmptyView()
            }
            Text(label)
                .font(.figtree(ThemeTypography.label, weight: .medium))
                .foregroundStyle(Theme.pillText)
                .lineLimit(ThemePill.previewLines)
                .fixedSize(horizontal: false, vertical: true)
            if !model.preview.text.isEmpty { Spacer(minLength: 0) }
        }
    }

    private var label: String {
        switch model.state {
        case .hidden: ""
        case .listening: "Luistert…"
        case .handsFree: "Handenvrij — tik om te stoppen"
        case .command: "Opdracht…"
        case .working: "Bezig…"
        case .error(let message): message
        }
    }
}

private struct CaptionTail: NSViewRepresentable {
    let text: String

    func makeNSView(context: Context) -> CaptionTailView { CaptionTailView() }

    func updateNSView(_ view: CaptionTailView, context: Context) {
        view.text = text
        view.needsDisplay = true
    }

    func sizeThatFits(_ proposal: ProposedViewSize, nsView: CaptionTailView, context: Context) -> CGSize? {
        let width = proposal.width ?? ThemePill.windowWidth - 2 * (ThemePill.outerInset + ThemeSpacing.md)
        return CGSize(width: width, height: min(nsView.textHeight(width: width), ThemePill.previewLineHeight * CGFloat(ThemePill.previewLines)))
    }
}

final class CaptionTailView: NSView {
    var text = ""
    override var isFlipped: Bool { true }

    private var attributedText: NSAttributedString {
        let paragraph = NSMutableParagraphStyle()
        paragraph.minimumLineHeight = ThemePill.previewLineHeight
        paragraph.maximumLineHeight = ThemePill.previewLineHeight
        paragraph.lineBreakMode = .byWordWrapping
        let font = NSFont(name: ThemeTypography.ui + "-Regular", size: ThemeTypography.content)
            ?? NSFont.systemFont(ofSize: ThemeTypography.content)
        return NSAttributedString(string: text, attributes: [.font: font, .foregroundColor: NSColor(Theme.pillText), .paragraphStyle: paragraph])
    }

    func textHeight(width: CGFloat) -> CGFloat {
        ceil(attributedText.boundingRect(with: NSSize(width: max(0, width), height: .greatestFiniteMagnitude), options: [.usesLineFragmentOrigin, .usesFontLeading]).height)
    }

    override func draw(_ dirtyRect: NSRect) {
        NSGraphicsContext.saveGraphicsState()
        defer { NSGraphicsContext.restoreGraphicsState() }
        NSBezierPath(rect: bounds).addClip()
        let height = textHeight(width: bounds.width)
        // A bottom-aligned viewport retains the latest lines when partial speech is revised.
        attributedText.draw(with: NSRect(x: 0, y: min(0, bounds.height - height), width: bounds.width, height: height), options: [.usesLineFragmentOrigin, .usesFontLeading])
    }
}

private struct WorkIndicator: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var spinning = false

    var body: some View {
        ZStack {
            Circle().inset(by: 1).stroke(Theme.pillAccent.opacity(0.35), lineWidth: 2)
            Circle().inset(by: 1).trim(from: 0.75, to: 1).stroke(Theme.pillAccent, lineWidth: 2)
        }
        .frame(width: ThemeSpacing.md, height: ThemeSpacing.md)
        .rotationEffect(.degrees(spinning ? 360 : 0))
        .animation(reduceMotion ? nil : .linear(duration: 0.8).repeatForever(autoreverses: false), value: spinning)
        .onAppear { spinning = !reduceMotion }
        .onChange(of: reduceMotion) { _, reduced in spinning = !reduced }
        .accessibilityHidden(true)
    }
}

private struct Wave: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let level: Float
    private let weights: [CGFloat] = [0.5, 0.8, 1, 0.75, 0.45]

    var body: some View {
        HStack(alignment: .center, spacing: 2.5) {
            ForEach(weights.indices, id: \.self) { i in
                Capsule()
                    .fill(Theme.pillAccent)
                    .frame(width: 2.5, height: 4 + 12 * weights[i] * min(1, CGFloat(level) * 12))
            }
        }
        .frame(height: 16)
        .animation(reduceMotion ? nil : .easeOut(duration: 0.12), value: level)
    }
}
