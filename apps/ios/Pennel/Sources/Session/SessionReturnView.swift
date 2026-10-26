import PennelMobileKit
import SwiftUI

/// Shown after the keyboard opened `pennel://sessie/start`: iOS has no API to return to the previous app, so the
/// user goes back with the system arrow in the top-left corner.
struct SessionReturnView: View {
    let session: DictationSession

    var body: some View {
        VStack(spacing: Brand.Spacing.s6) {
            Spacer()
            SessionWave(active: session.phase != .off)
                .frame(width: 72, height: 48)
            Group {
                if session.phase != .off {
                    Text(Copy.Keyboard.micReadyInApp)
                } else if let error = session.startError {
                    Text(error).foregroundStyle(.red)
                } else {
                    ProgressView()
                }
            }
            .font(Theme.font(.title3, weight: .medium))
            .multilineTextAlignment(.center)
            Spacer()
            if session.phase != .off {
                Button(Copy.Dictation.stopSession) { session.stop() }
                    .buttonStyle(.bordered)
                    .font(Theme.font(.body, weight: .medium))
            }
        }
        .padding(Brand.Spacing.s8)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Brand.surface.color)
    }
}

private struct SessionWave: View {
    let active: Bool

    var body: some View {
        GeometryReader { proxy in
            let heights = Brand.Wave.barHeights(level: 0, height: proxy.size.height)
            HStack(alignment: .center, spacing: proxy.size.width * Brand.Wave.spacingRatio) {
                ForEach(heights.indices, id: \.self) { index in
                    Capsule()
                        .fill(active ? Brand.accent.color : Brand.muted.color)
                        .frame(width: proxy.size.width * Brand.Wave.barWidthRatio, height: heights[index])
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .accessibilityHidden(true)
    }
}
