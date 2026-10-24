@preconcurrency import MusliCore
import Observation
import SwiftUI

/// A conversation that outlives its view, so switching tabs or opening a source keeps it.
@MainActor
@Observable
final class MeetingChat {
    struct Exchange: Identifiable {
        let id = UUID()
        let question: String
        var answer: MeetingAnswer?
        var error: String?
    }

    let meetingId: String?
    private(set) var exchanges: [Exchange] = []

    init(meetingId: String?) {
        self.meetingId = meetingId
    }

    var busy: Bool { exchanges.last.map { $0.answer == nil && $0.error == nil } ?? false }

    func ask(_ question: String, engine: MusliEngine, keys: ApiKeys) {
        guard !question.isEmpty, !busy else { return }
        let history = exchanges.compactMap { exchange in
            exchange.answer.map { ChatTurn(question: exchange.question, answer: $0.text) }
        }
        let exchange = Exchange(question: question)
        exchanges.append(exchange)
        let scope = meetingId
        Task {
            let result = await Task.detached {
                Result { try engine.askMeetings(question: question, history: history, meetingId: scope, keys: keys) }
            }.value
            guard let index = exchanges.firstIndex(where: { $0.id == exchange.id }) else { return }
            switch result {
            case .success(let answer): exchanges[index].answer = answer
            case .failure(let error):
                exchanges[index].error = Self.message(error)
                Diagnostics("chat").error("vraag over \(scope ?? "alle meetings") mislukt: \(error)")
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
}

struct TranscriptJump: Equatable {
    let meetingId: String
    let ms: UInt64
    let token = UUID()
}
