import Foundation
import Observation
import PennelMobileKit

@MainActor
@Observable
final class KeyboardTypingModel {
    var state = KeyboardTypingState()
    var suggestions: [String] = []
    var suggestionsEnabled: Bool
    var returnKey: KeyboardCopy.Key = .returnKey
    var returnEnabled = true
    var allowsPeriod = true
    var showsAt = false
    var showsURL = false
    let copy: KeyboardCopy
    private var index = KeyboardSuggestionIndex(words: [])
    private(set) var currentPrefix = ""

    init(copy: KeyboardCopy, suggestionsEnabled: Bool) {
        self.copy = copy
        self.suggestionsEnabled = suggestionsEnabled
    }

    func setLexicon(_ entries: [KeyboardLexiconEntry], words: [String]) {
        index = KeyboardSuggestionIndex(words: words, supplementary: entries)
        suggestions = suggestionsEnabled ? index.suggestions(for: currentPrefix) : []
    }

    func update(before: String?, capitalization: KeyboardCapitalization) {
        state.updateCapitalization(before: before, mode: capitalization)
        currentPrefix = String((before ?? "").suffix(64).reversed().prefix { $0.isLetter || $0.isNumber || $0 == "'" }.reversed())
        suggestions = suggestionsEnabled ? index.suggestions(for: currentPrefix) : []
    }
}
