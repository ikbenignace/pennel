@preconcurrency import MusliCore
import MusliKit
import SwiftUI

extension Notification.Name {
    static let musliShowTranscriptAt = Notification.Name("dev.musli.showTranscriptAt")
}

struct MeetingChatView: View {
    let state: AppState
    let meetingId: String?

    private struct Exchange: Identifiable {
        let id = UUID()
        let question: String
        var answer: MeetingAnswer?
        var error: String?
    }

    @State private var exchanges: [Exchange] = []
    @State private var draft = ""
    @FocusState private var focused: Bool

    private var busy: Bool { exchanges.last.map { $0.answer == nil && $0.error == nil } ?? false }

    var body: some View {
        VStack(spacing: 0) {
            ScrollViewReader { proxy in
                ScrollView {
                    VStack(alignment: .leading, spacing: 18) {
                        if exchanges.isEmpty { placeholder }
                        ForEach(exchanges) { exchange in
                            exchangeView(exchange).id(exchange.id)
                        }
                    }
                    .padding(.horizontal, 32)
                    .padding(.vertical, 16)
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .onChange(of: exchanges.count) {
                    if let last = exchanges.last { withAnimation { proxy.scrollTo(last.id, anchor: .bottom) } }
                }
            }
            HStack(spacing: 8) {
                TextField(meetingId == nil ? "Vraag iets over al je meetings" : "Vraag iets over deze meeting", text: $draft)
                    .textFieldStyle(.roundedBorder)
                    .focused($focused)
                    .onSubmit(ask)
                    .disabled(busy)
                Button("Vraag", action: ask).disabled(busy || draft.trimmingCharacters(in: .whitespaces).isEmpty)
            }
            .padding(.horizontal, 32)
            .padding(.vertical, 14)
        }
        .onAppear { focused = true }
    }

    private var placeholder: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(meetingId == nil ? "Vraag het je meetings" : "Vraag het deze meeting")
                .font(.custom("Instrument Serif", size: 26))
            Text(meetingId == nil
                 ? "Bijvoorbeeld: \"Wat hebben we beslist over het budget?\" of \"Wanneer sprak ik laatst met Jan?\""
                 : "Bijvoorbeeld: \"Wat zijn de actiepunten voor mij?\"")
                .font(.figtree(12))
                .foregroundStyle(Theme.muted)
        }
    }

    private func exchangeView(_ exchange: Exchange) -> some View {
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
                Text(Self.markdown(answer.text))
                    .font(.figtree(14))
                    .textSelection(.enabled)
                    .fixedSize(horizontal: false, vertical: true)
                if !answer.sources.isEmpty { sources(answer.sources) }
            } else if let error = exchange.error {
                Text(error).font(.figtree(12)).foregroundStyle(Theme.accent)
            } else {
                ProgressView().controlSize(.small)
            }
        }
    }

    private func sources(_ passages: [Passage]) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            ForEach(Array(passages.enumerated()), id: \.offset) { index, passage in
                Button {
                    open(passage)
                } label: {
                    Text("\(index + 1). \(Self.label(passage))")
                        .font(.figtree(11))
                        .foregroundStyle(Theme.muted)
                        .underline()
                }
                .buttonStyle(.plain)
            }
        }
    }

    private func open(_ passage: Passage) {
        NotificationCenter.default.post(name: .musliSelectMeeting, object: passage.meetingId)
        if passage.kind == .transcript, let ms = passage.startMs {
            DispatchQueue.main.async {
                NotificationCenter.default.post(name: .musliShowTranscriptAt, object: ms)
            }
        }
    }

    private func ask() {
        let question = draft.trimmingCharacters(in: .whitespaces)
        guard !question.isEmpty, !busy else { return }
        draft = ""
        let history = exchanges.compactMap { exchange in
            exchange.answer.map { ChatTurn(question: exchange.question, answer: $0.text) }
        }
        exchanges.append(Exchange(question: question))
        let (engine, keys, scope) = (state.engine, state.keys(), meetingId)
        Task {
            let result = await Task.detached {
                Result { try engine.askMeetings(question: question, history: history, meetingId: scope, keys: keys) }
            }.value
            guard let index = exchanges.indices.last else { return }
            switch result {
            case .success(let answer): exchanges[index].answer = answer
            case .failure(let error): exchanges[index].error = Self.message(error)
            }
        }
    }

    private static func message(_ error: Error) -> String {
        switch error as? MusliError {
        case .MissingApiKey, .NotFound: "Geen taalmodel ingesteld — kies er een bij Instellingen → Modellen."
        case .Network: "Geen verbinding met je taalmodel."
        default: "Vragen mislukt: \(error)"
        }
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

    static func markdown(_ text: String) -> AttributedString {
        (try? AttributedString(markdown: text, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)))
            ?? AttributedString(text)
    }
}
