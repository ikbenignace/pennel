@preconcurrency import PennelCore
import PennelKit
import SwiftUI

struct MeetingChatView: View {
    let state: AppState
    let chat: MeetingChat
    private var draft: String {
        get { state.workspace.chatDrafts[chat.meetingId ?? ""] ?? "" }
        nonmutating set { state.workspace.chatDrafts[chat.meetingId ?? ""] = newValue }
    }
    @FocusState private var focused: Bool

    private var meetingId: String? { chat.meetingId }

    var body: some View {
        VStack(spacing: 0) {
            ScrollViewReader { proxy in
                ScrollView {
                    VStack(alignment: .leading, spacing: 18) {
                        if chat.exchanges.isEmpty { placeholder }
                        ForEach(chat.exchanges) { exchange in
                            exchangeView(exchange).id(exchange.id)
                        }
                    }
                    .padding(.horizontal, 32)
                    .padding(.vertical, 16)
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .onChange(of: chat.exchanges.count, initial: true) {
                    if let last = chat.exchanges.last { proxy.scrollTo(last.id, anchor: .bottom) }
                }
            }
            HStack(spacing: 8) {
                TextField(meetingId == nil ? "Vraag iets over al je meetings" : "Vraag iets over deze meeting", text: Binding(get: { draft }, set: { draft = $0 }))
                    .textFieldStyle(.roundedBorder)
                    .focused($focused)
                    .onSubmit(ask)
                    .disabled(chat.busy)
                Button("Vraag", action: ask).disabled(chat.busy || draft.trimmingCharacters(in: .whitespaces).isEmpty)
            }
            .padding(.horizontal, 32)
            .padding(.vertical, 14)
        }
        .onAppear { focused = true }
    }

    private var placeholder: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(meetingId == nil ? "Vraag het je meetings" : "Vraag het deze meeting")
                .font(.figtree(22, weight: .semibold))
            Text(meetingId == nil
                 ? "Bijvoorbeeld: \"Wat hebben we beslist over het budget?\" of \"Wanneer sprak ik laatst met Jan?\""
                 : "Bijvoorbeeld: \"Wat zijn de actiepunten voor mij?\"")
                .font(.figtree(12))
                .foregroundStyle(Theme.muted)
        }
    }

    private func exchangeView(_ exchange: MeetingChat.Exchange) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Spacer()
                Text(exchange.question)
                    .font(.figtree(13))
                    .padding(.horizontal, 12)
                    .padding(.vertical, 7)
                    .background(RoundedRectangle(cornerRadius: 10).fill(Theme.line))
            }
            if let answer = exchange.answer {
                MarkdownBlock(text: answer.text)
                if !answer.citations.isEmpty { sources(answer.citations) }
            } else if let error = exchange.error {
                Text(error).font(.figtree(12)).foregroundStyle(Theme.accent)
            } else {
                ProgressView().controlSize(.small)
            }
        }
    }

    private func sources(_ citations: [Citation]) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            ForEach(citations, id: \.number) { citation in
                Button {
                    open(citation.passage)
                } label: {
                    Text("[\(citation.number)] \(Self.label(citation.passage))")
                        .font(.figtree(11))
                        .foregroundStyle(Theme.muted)
                        .underline()
                }
                .buttonStyle(.plain)
            }
        }
    }

    private func open(_ passage: Passage) {
        if passage.kind == .transcript, let ms = passage.startMs {
            state.transcriptJump = TranscriptJump(meetingId: passage.meetingId, ms: ms)
        }
        NotificationCenter.default.post(name: .pennelSelectMeeting, object: passage.meetingId)
    }

    private func ask() {
        let question = draft.trimmingCharacters(in: .whitespaces)
        guard !question.isEmpty, !chat.busy else { return }
        draft = ""
        chat.ask(question, engine: state.engine, keys: state.keys())
    }

    static func label(_ passage: Passage) -> String {
        let date = LibraryView.date(MeetingInfo(id: "", title: "", startIso: passage.startIso, durationMin: 0, hasSummary: false))
        let day = date?.formatted(date: .abbreviated, time: .omitted) ?? ""
        let place: String = switch passage.kind {
        case .transcript: passage.startMs.map { TranscriptList.stamp($0) } ?? "transcript"
        case .notes: "notities"
        case .summary: "samenvatting"
        }
        return "\(passage.title) · \(day) · \(place)"
    }
}

/// Renders the block Markdown an LLM answers with: headings, bullets and paragraphs with inline styling.
struct MarkdownBlock: View {
    let text: String

    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            ForEach(Array(text.components(separatedBy: "\n").enumerated()), id: \.offset) { _, line in
                lineView(line.trimmingCharacters(in: .whitespaces))
            }
        }
        .textSelection(.enabled)
        .fixedSize(horizontal: false, vertical: true)
    }

    @ViewBuilder
    private func lineView(_ line: String) -> some View {
        if line.isEmpty {
            Color.clear.frame(height: 2)
        } else if let heading = Self.heading(line) {
            Text(Self.inline(heading)).font(.figtree(14, weight: .semibold)).padding(.top, 4)
        } else if let item = ["- ", "* ", "• "].first(where: line.hasPrefix).map({ String(line.dropFirst($0.count)) }) {
            HStack(alignment: .firstTextBaseline, spacing: 6) {
                Text("•").foregroundStyle(Theme.muted)
                Text(Self.inline(item))
            }
            .font(.figtree(14))
        } else {
            Text(Self.inline(line)).font(.figtree(14))
        }
    }

    static func heading(_ line: String) -> String? {
        guard line.hasPrefix("#") else { return nil }
        let rest = line.drop(while: { $0 == "#" })
        guard rest.hasPrefix(" ") else { return nil }
        return rest.trimmingCharacters(in: .whitespaces)
    }

    static func inline(_ text: String) -> AttributedString {
        (try? AttributedString(markdown: text, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)))
            ?? AttributedString(text)
    }
}
