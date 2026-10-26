import PennelMobileKit
import SwiftUI

struct DictationPad: View {
    let model: KeyboardModel
    let openSettings: () -> Void

    var body: some View {
        VStack(spacing: Brand.Spacing.s2) {
            switch model.pad {
            case .needsFullAccess:
                Text(Copy.Keyboard.fullAccessNeeded)
                    .font(.figtree(Brand.FontSize.content, weight: .medium))
                    .foregroundStyle(Brand.foreground.color)
                    .multilineTextAlignment(.center)
                Button(Copy.Keyboard.openSettings, action: openSettings)
                    .buttonStyle(PadButtonStyle())
            case .openPennelFallback:
                Text(Copy.Keyboard.openPennelFallback)
                    .font(.figtree(Brand.FontSize.content, weight: .medium))
                    .foregroundStyle(Brand.foreground.color)
                    .multilineTextAlignment(.center)
            case .failed(let message):
                Text(message)
                    .font(.figtree(Brand.FontSize.content, weight: .medium))
                    .foregroundStyle(Brand.foreground.color)
                    .multilineTextAlignment(.center)
                Button(Copy.Keyboard.retry) { model.retry() }
                    .buttonStyle(PadButtonStyle())
            case .sessionOff:
                pill(label: Copy.Keyboard.startPennel)
                caption(Copy.Keyboard.startPennelHint)
            case .ready:
                pill(label: nil)
                caption(Copy.Keyboard.ready)
            case .listening:
                pill(label: nil)
                HStack(spacing: Brand.Spacing.s3) {
                    caption(Copy.Keyboard.listening)
                    Button { model.cancel() } label: {
                        Image(systemName: "xmark.circle.fill")
                            .font(.system(size: 22))
                            .foregroundStyle(Brand.muted.color)
                    }
                    .accessibilityLabel("Annuleren")
                }
            case .processing:
                pill(label: nil)
                caption(Copy.Keyboard.processing)
            }
            if model.lastResult != nil, model.pad != .listening, model.pad != .processing {
                Button(Copy.Keyboard.insertLast) { model.insertLast() }
                    .buttonStyle(PadButtonStyle())
            }
        }
        .frame(maxWidth: .infinity)
    }

    private func caption(_ text: String) -> some View {
        Text(text)
            .font(.figtree(Brand.FontSize.body, weight: .medium))
            .foregroundStyle(Brand.muted.color)
            .multilineTextAlignment(.center)
    }

    private func pill(label: String?) -> some View {
        let active = model.pad == .listening
        return ZStack {
            Capsule().fill(Brand.pillBackground.color)
            if let label {
                Text(label)
                    .font(.figtree(Brand.FontSize.section, weight: .semibold))
                    .foregroundStyle(Brand.pillText.color)
            } else {
                Bars(level: active ? model.level : 0)
                    .frame(height: 36)
            }
        }
        .frame(height: 76)
        .padding(.horizontal, Brand.Spacing.s6)
        .contentShape(Capsule())
        .onTapGesture { model.tapPill() }
        .onLongPressGesture(minimumDuration: 0.3, maximumDistance: 60) {
            model.holdBegan()
        } onPressingChanged: { pressing in
            if !pressing { model.holdEnded() }
        }
        .accessibilityAddTraits(.isButton)
        .accessibilityLabel(label ?? Copy.Keyboard.ready)
    }
}

private struct Bars: View {
    let level: Float

    var body: some View {
        GeometryReader { proxy in
            let heights = Brand.Wave.barHeights(level: level, height: proxy.size.height)
            let width = proxy.size.height * Brand.Wave.barWidthRatio
            HStack(spacing: proxy.size.height * Brand.Wave.spacingRatio) {
                ForEach(heights.indices, id: \.self) { index in
                    Capsule()
                        .fill(Brand.pillAccent.color)
                        .frame(width: width, height: heights[index])
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .animation(.easeOut(duration: 0.12), value: level)
        }
    }
}

struct PadButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.figtree(Brand.FontSize.content, weight: .semibold))
            .foregroundStyle(Brand.buttonText.color)
            .padding(.horizontal, Brand.Spacing.s5)
            .frame(minHeight: 40)
            .background(Brand.button.color.opacity(configuration.isPressed ? 0.8 : 1), in: Capsule())
    }
}
