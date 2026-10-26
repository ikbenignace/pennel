import os
import PennelMobileKit
import SwiftUI

/// Opens a meeting, optionally at a moment in its transcript (a cited source).
struct MeetingRoute: Hashable {
    var id: String
    var startMs: UInt64?
}

/// The **Vraag** tab: questions over all meetings (or one, inside a meeting) with sources that open the meeting.
struct AskView: View {
    var body: some View {
        NavigationStack {
            AskChat(meetingId: nil)
                .navigationTitle(Copy.Tab.ask)
                .navigationDestination(for: MeetingRoute.self) { route in
                    MeetingDetailView(id: route.id, jumpMs: route.startMs)
                }
        }
    }
}

struct AskChat: View {
    @Environment(AppModel.self) private var model
    let meetingId: String?

    @State private var exchanges: [Exchange] = []
    @State private var draft = ""
    private static let log = Logger(subsystem: "app.pennel.ios", category: "ask")

    struct Exchange: Identifiable {
        let id = UUID()
        let question: String
        var answer: MeetingAnswer?
        var error: String?
    }

    private var busy: Bool { exchanges.contains { $0.answer == nil && $0.error == nil } }

    var body: some View {
        List {
            if exchanges.isEmpty {
                VStack(alignment: .leading, spacing: Brand.Spacing.s2) {
                    Text(meetingId == nil ? Copy.Ask.titleAll : Copy.Ask.titleMeeting)
                        .font(.instrumentSerif(Brand.FontSize.pageTitle))
                    Text(meetingId == nil ? Copy.Ask.exampleAll : Copy.Ask.exampleMeeting)
                        .foregroundStyle(Brand.muted.color)
                }
                .listRowBackground(Color.clear)
            }
            ForEach(exchanges) { exchange in
                VStack(alignment: .leading, spacing: Brand.Spacing.s2) {
                    Text(exchange.question).font(Theme.font(.body, weight: .semibold))
                    if let answer = exchange.answer {
                        MarkdownText(answer.text)
                        ForEach(answer.citations, id: \.number) { citation in
                            NavigationLink(value: MeetingRoute(id: citation.passage.meetingId, startMs: citation.passage.startMs)) {
                                Text("[\(citation.number)] \(Self.label(citation.passage))")
                                    .font(Theme.font(.footnote))
                                    .foregroundStyle(Brand.accent.color)
                            }
                        }
                    } else if let error = exchange.error {
                        Text(error).foregroundStyle(.red)
                    } else {
                        ProgressView().accessibilityLabel(Copy.Ask.writing)
                    }
                }
                .pennelRow()
            }
        }
        .pennelList()
        .safeAreaInset(edge: .bottom) {
            HStack(spacing: Brand.Spacing.s2) {
                TextField(meetingId == nil ? Copy.Ask.placeholderAll : Copy.Ask.placeholderMeeting, text: $draft, axis: .vertical)
                    .textFieldStyle(.roundedBorder)
                    .lineLimit(1...4)
                    .onSubmit(ask)
                Button(busy ? Copy.Meetings.busy : Copy.Ask.ask, action: ask)
                    .buttonStyle(.borderedProminent)
                    .disabled(busy || draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
            .padding(Brand.Spacing.s3)
            .background(Brand.surface.color)
        }
    }

    private func ask() {
        let question = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !question.isEmpty, !busy else { return }
        draft = ""
        let history = exchanges.compactMap { exchange in
            exchange.answer.map { ChatTurn(question: exchange.question, answer: $0.text) }
        }
        exchanges.append(Exchange(question: question))
        let index = exchanges.count - 1
        let engine = model.engine
        let meetingId = meetingId
        Task {
            let result = await Task.detached { () -> Result<MeetingAnswer, Error> in
                Result { try engine.askMeetings(question: question, history: history, meetingId: meetingId, keys: Keychain.apiKeys()) }
            }.value
            switch result {
            case .success(let answer): exchanges[index].answer = answer
            case .failure(let error):
                Self.log.error("ask failed: \(ErrorText.code(error), privacy: .public)")
                exchanges[index].error = ErrorText.failed(error)
            }
        }
    }

    /// The desktop's `citationLabel`: title · date · place.
    static func label(_ passage: Passage) -> String {
        let place: String = switch passage.kind {
        case .transcript: passage.startMs.map { Copy.minutesAndSeconds(Double($0) / 1000) } ?? "transcript"
        case .summary: "samenvatting"
        case .notes: "notities"
        }
        return "\(passage.title) · \(MeetingDates.dayLabel(passage.startIso)) · \(place)"
    }
}

/// Markdown paragraphs as native text, one block per line.
struct MarkdownText: View {
    let text: String

    init(_ text: String) {
        self.text = text
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Brand.Spacing.s1) {
            ForEach(Array(text.components(separatedBy: "\n").enumerated()), id: \.offset) { _, line in
                if !line.trimmingCharacters(in: .whitespaces).isEmpty {
                    MarkdownLine(line: line)
                }
            }
        }
    }
}

struct MarkdownLine: View {
    let line: String

    var body: some View {
        let trimmed = line.trimmingCharacters(in: .whitespaces)
        if trimmed.hasPrefix("#") {
            Text(inline(trimmed.drop { $0 == "#" }.trimmingCharacters(in: .whitespaces)))
                .font(Theme.font(.headline, weight: .semibold))
        } else if trimmed.hasPrefix("- ") || trimmed.hasPrefix("* ") {
            HStack(alignment: .firstTextBaseline, spacing: Brand.Spacing.s2) {
                Text("•")
                Text(inline(String(trimmed.dropFirst(2))))
            }
        } else {
            Text(inline(trimmed))
        }
    }

    private func inline(_ text: String) -> AttributedString {
        (try? AttributedString(markdown: text, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)))
            ?? AttributedString(text)
    }
}
