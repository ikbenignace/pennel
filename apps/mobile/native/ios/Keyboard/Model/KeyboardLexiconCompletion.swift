import PennelMobileKit
import UIKit

enum KeyboardLexiconCompletion {
    @MainActor
    static func make(
        apply: @escaping @MainActor @Sendable ([KeyboardLexiconEntry]) -> Void
    ) -> @Sendable (UILexicon) -> Void {
        // UIKit may invoke this completion on its lexicon worker queue.
        { lexicon in
            Task { @MainActor in
                let entries = lexicon.entries.prefix(2_048).map {
                    KeyboardLexiconEntry(input: $0.userInput, replacement: $0.documentText)
                }
                apply(entries)
            }
        }
    }
}
