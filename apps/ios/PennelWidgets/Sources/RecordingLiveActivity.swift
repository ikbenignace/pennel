import ActivityKit
import AppIntents
import PennelMobileKit
import SwiftUI
import WidgetKit

/// Lock Screen and Dynamic Island while a meeting records: bars, the running time and **Stop opname**.
struct RecordingLiveActivity: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: RecordingActivityAttributes.self) { context in
            HStack(spacing: Brand.Spacing.s4) {
                RecordingBars(state: context.state)
                    .frame(width: 36, height: 28)
                RecordingStatus(state: context.state)
                    .font(.figtree(Brand.FontSize.section, weight: .medium))
                    .foregroundStyle(Brand.pillText.color)
                Spacer(minLength: 0)
                RecordingStopButton(state: context.state)
            }
            .padding(Brand.Spacing.s4)
            .activityBackgroundTint(Brand.pillBackground.color)
            .activitySystemActionForegroundColor(Brand.pillText.color)
        } dynamicIsland: { context in
            DynamicIsland {
                DynamicIslandExpandedRegion(.leading) {
                    RecordingBars(state: context.state).frame(width: 36, height: 28)
                }
                DynamicIslandExpandedRegion(.center) {
                    RecordingStatus(state: context.state)
                        .font(.figtree(Brand.FontSize.content, weight: .medium))
                }
                DynamicIslandExpandedRegion(.trailing) {
                    RecordingStopButton(state: context.state)
                }
            } compactLeading: {
                RecordingBars(state: context.state).frame(width: 20, height: 14)
            } compactTrailing: {
                RecordingClock(startedAt: context.state.startedAt)
                    .font(.figtree(Brand.FontSize.label, weight: .medium))
                    .frame(maxWidth: 52)
            } minimal: {
                RecordingBars(state: context.state).frame(width: 18, height: 12)
            }
            .keylineTint(Brand.wave.color)
        }
    }
}

private struct RecordingStatus: View {
    let state: RecordingActivityState

    var body: some View {
        switch state.phase {
        case .recording:
            HStack(spacing: 0) {
                Text("\(Copy.Meetings.recordingRow) · ")
                RecordingClock(startedAt: state.startedAt)
            }
        case .interrupted:
            Text(state.message ?? Copy.Meetings.recordingRow)
                .lineLimit(2)
        case .finishing:
            Text(Copy.Meetings.finishing)
        }
    }
}

private struct RecordingClock: View {
    let startedAt: Date

    var body: some View {
        Text(timerInterval: startedAt...Date.distantFuture, countsDown: false)
            .monospacedDigit()
    }
}

private struct RecordingStopButton: View {
    let state: RecordingActivityState

    var body: some View {
        if state.phase != .finishing {
            Button(intent: StopRecordingIntent()) {
                Text(Copy.Meetings.stopRecording)
                    .font(.figtree(Brand.FontSize.content, weight: .semibold))
            }
            .tint(Brand.wave.color)
        }
    }
}

private struct RecordingBars: View {
    let state: RecordingActivityState

    var body: some View {
        GeometryReader { proxy in
            let heights = Brand.Wave.barHeights(level: state.phase == .recording ? 0.5 : 0, height: proxy.size.height)
            HStack(alignment: .center, spacing: proxy.size.width * Brand.Wave.spacingRatio) {
                ForEach(heights.indices, id: \.self) { index in
                    Capsule()
                        .fill(Brand.wave.color.opacity(state.phase == .recording ? 1 : 0.4))
                        .frame(width: proxy.size.width * Brand.Wave.barWidthRatio, height: heights[index])
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .accessibilityHidden(true)
    }
}
