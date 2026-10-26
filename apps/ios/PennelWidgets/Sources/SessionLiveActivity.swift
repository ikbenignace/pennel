import ActivityKit
import AppIntents
import PennelMobileKit
import SwiftUI
import WidgetKit

/// Lock Screen and Dynamic Island for the keyboard dictation session: soft-wave bars on ink, the status with a
/// live countdown, and Stop.
struct SessionLiveActivity: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: SessionActivityAttributes.self) { context in
            HStack(spacing: Brand.Spacing.s4) {
                SessionBars(state: context.state)
                    .frame(width: 36, height: 28)
                SessionStatus(state: context.state)
                    .font(.figtree(Brand.FontSize.section, weight: .medium))
                    .foregroundStyle(Brand.pillText.color)
                Spacer(minLength: 0)
                SessionStopButton(state: context.state)
            }
            .padding(Brand.Spacing.s4)
            .activityBackgroundTint(Brand.pillBackground.color)
            .activitySystemActionForegroundColor(Brand.pillText.color)
        } dynamicIsland: { context in
            DynamicIsland {
                DynamicIslandExpandedRegion(.leading) {
                    SessionBars(state: context.state).frame(width: 36, height: 28)
                }
                DynamicIslandExpandedRegion(.center) {
                    SessionStatus(state: context.state)
                        .font(.figtree(Brand.FontSize.content, weight: .medium))
                }
                DynamicIslandExpandedRegion(.trailing) {
                    SessionStopButton(state: context.state)
                }
            } compactLeading: {
                SessionBars(state: context.state).frame(width: 20, height: 14)
            } compactTrailing: {
                if context.state.phase == .ready {
                    SessionCountdown(expiresAt: context.state.expiresAt)
                        .font(.figtree(Brand.FontSize.label, weight: .medium))
                        .frame(maxWidth: 44)
                }
            } minimal: {
                SessionBars(state: context.state).frame(width: 18, height: 12)
            }
            .keylineTint(Brand.wave.color)
        }
    }
}

private struct SessionStatus: View {
    let state: SessionActivityState

    var body: some View {
        switch state.phase {
        case .ready:
            Text("\(Copy.Keyboard.micReadyLiveActivity) · \(SessionCountdown.text(until: state.expiresAt))")
                .monospacedDigit()
        case .listening:
            Text(Copy.Keyboard.listening)
        case .processing:
            Text(Copy.Dictation.processing)
        case .off:
            Text(state.message ?? Copy.Keyboard.sessionOff)
        }
    }
}

private struct SessionCountdown: View {
    let expiresAt: Date

    var body: some View {
        Self.text(until: expiresAt).monospacedDigit().multilineTextAlignment(.trailing)
    }

    /// The system counts down without updates from the app; the range must not start after its end.
    static func text(until expiresAt: Date) -> Text {
        Text(timerInterval: min(Date(), expiresAt)...expiresAt, countsDown: true)
    }
}

private struct SessionStopButton: View {
    let state: SessionActivityState

    var body: some View {
        if state.phase != .off {
            Button(intent: StopDictationSessionIntent()) {
                Text(Copy.Session.stop)
                    .font(.figtree(Brand.FontSize.content, weight: .semibold))
            }
            .tint(Brand.wave.color)
        }
    }
}

private struct SessionBars: View {
    let state: SessionActivityState

    var body: some View {
        GeometryReader { proxy in
            let level = state.phase == .listening ? max(state.level, 0.5) : 0
            let heights = Brand.Wave.barHeights(level: level, height: proxy.size.height)
            HStack(alignment: .center, spacing: proxy.size.width * Brand.Wave.spacingRatio) {
                ForEach(heights.indices, id: \.self) { index in
                    Capsule()
                        .fill(Brand.wave.color.opacity(state.phase == .off ? 0.4 : 1))
                        .frame(width: proxy.size.width * Brand.Wave.barWidthRatio, height: heights[index])
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .accessibilityHidden(true)
    }
}
