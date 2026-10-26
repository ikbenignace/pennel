import PennelMobileKit
import SwiftUI

/// Recording (spec §4.1): editable title, time, wave, **Je notities**, folded **Live transcript**, **Stop opname**.
struct MeetingLiveView: View {
    let runtime: MeetingRuntime
    /// Called when the view should close; with the id of the meeting that was just finished, if any.
    let onDone: (String?) -> Void

    @State private var title = ""
    @State private var notes = ""
    @State private var saveNotes: Task<Void, Never>?
    @State private var showsTranscript = false
    @FocusState private var editingTitle: Bool

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: Brand.Spacing.s4) {
                    TextField(Copy.Meetings.meetingTitle, text: $title)
                        .font(.instrumentSerif(Brand.FontSize.displayTitle))
                        .focused($editingTitle)
                        .disabled(!runtime.isRecording)
                        .onChange(of: title) { runtime.rename(title) }
                        .accessibilityLabel(Copy.Meetings.meetingTitle)

                    HStack(spacing: Brand.Spacing.s3) {
                        WaveMark(level: runtime.phase == .recording ? 0.6 : 0)
                            .frame(width: 32, height: 24)
                        status
                        Spacer()
                    }

                    if let warning = runtime.warning {
                        Text(warning)
                            .font(Theme.font(.footnote))
                            .foregroundStyle(runtime.phase == .interrupted ? Brand.muted.color : .red)
                    }

                    Text(Copy.Meetings.yourNotes).font(Theme.font(.subheadline, weight: .semibold))
                    TextEditor(text: $notes)
                        .frame(minHeight: 200)
                        .scrollContentBackground(.hidden)
                        .padding(Brand.Spacing.s2)
                        .background(Brand.field.color, in: RoundedRectangle(cornerRadius: Brand.Radius.control))
                        .overlay(alignment: .topLeading) {
                            if notes.isEmpty {
                                Text(Copy.Meetings.notesPlaceholder)
                                    .foregroundStyle(Brand.muted.color)
                                    .padding(Brand.Spacing.s3)
                                    .allowsHitTesting(false)
                            }
                        }
                        .disabled(!runtime.isRecording)
                        .onChange(of: notes) { scheduleNotesSave() }

                    DisclosureGroup(Copy.Meetings.liveTranscript, isExpanded: $showsTranscript) {
                        VStack(alignment: .leading, spacing: Brand.Spacing.s2) {
                            if runtime.transcript.isEmpty {
                                Text(Copy.Meetings.noTranscript).foregroundStyle(Brand.muted.color)
                            }
                            ForEach(runtime.transcript, id: \.startMs) { segment in
                                HStack(alignment: .firstTextBaseline, spacing: Brand.Spacing.s2) {
                                    Text(Copy.minutesAndSeconds(Double(segment.startMs) / 1000))
                                        .monospacedDigit()
                                        .foregroundStyle(Brand.muted.color)
                                    Text(segment.failed ? Copy.Meetings.transcriptionFailed : segment.text)
                                        .foregroundStyle(segment.failed ? Brand.muted.color : Brand.foreground.color)
                                }
                                .font(Theme.font(.footnote))
                            }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.top, Brand.Spacing.s2)
                    }
                    .font(Theme.font(.subheadline, weight: .semibold))
                }
                .padding(Brand.Spacing.s5)
            }
            .background(Brand.surface.color)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button { onDone(nil) } label: { Image(systemName: "chevron.down") }
                        .accessibilityLabel("Sluiten")
                }
            }
            .safeAreaInset(edge: .bottom) {
                Button(action: stop) {
                    Text(runtime.phase == .finishing ? Copy.Meetings.busy : Copy.Meetings.stopRecording)
                        .font(Theme.font(.headline, weight: .semibold))
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, Brand.Spacing.s2)
                }
                .buttonStyle(.borderedProminent)
                .tint(Brand.button.color)
                .controlSize(.large)
                .disabled(!runtime.isRecording)
                .padding(Brand.Spacing.s4)
                .background(Brand.surface.color)
            }
        }
        .onAppear {
            title = runtime.title
            notes = runtime.notes
        }
        .onChange(of: runtime.phase) {
            if runtime.phase == .idle { onDone(runtime.lastFinish?.id) }
        }
    }

    @ViewBuilder private var status: some View {
        switch runtime.phase {
        case .finishing:
            Text(Copy.Meetings.finishing(pending: runtime.pending))
                .foregroundStyle(Brand.muted.color)
        default:
            if let startedAt = runtime.startedAt {
                TimelineView(.periodic(from: startedAt, by: 1)) { context in
                    Text(Copy.Meetings.recordingClock(context.date.timeIntervalSince(startedAt)))
                        .monospacedDigit()
                }
            }
        }
    }

    private func scheduleNotesSave() {
        saveNotes?.cancel()
        let text = notes
        saveNotes = Task {
            try? await Task.sleep(for: .milliseconds(500))
            guard !Task.isCancelled else { return }
            runtime.setNotes(text)
        }
    }

    private func stop() {
        saveNotes?.cancel()
        runtime.setNotes(notes)
        runtime.rename(title)
        Task { await runtime.stop() }
    }
}
