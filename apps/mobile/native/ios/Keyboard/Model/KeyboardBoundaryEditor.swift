import PennelMobileKit
import UIKit

@MainActor
enum KeyboardBoundaryEditor {
    static func apply(original: String, replacement: String, delimiter: String,
                      proxy: any UITextDocumentProxy, lifetime: UUID) -> KeyboardCorrectionTransaction? {
        let document = proxy.documentIdentifier
        let before = proxy.documentContextBeforeInput
        let after = proxy.documentContextAfterInput
        guard proxy.selectedText?.isEmpty != false,
              after?.first.map({ !$0.isLetter && !$0.isNumber }) ?? true,
              let transaction = KeyboardCorrectionTransaction(original: original, replacement: replacement, delimiter: delimiter,
                  before: before, after: after, document: document, lifetime: lifetime) else { return nil }
        // Only the proven prefix is deleted. Missing following context is retained as unknown, never inferred to be EOF.
        for remaining in stride(from: original.count, through: 1, by: -1) {
            guard proxy.documentIdentifier == document, proxy.selectedText?.isEmpty != false,
                  proxy.documentContextBeforeInput?.hasSuffix(String(original.prefix(remaining))) == true else { return nil }
            proxy.deleteBackward()
        }
        guard proxy.documentIdentifier == document, proxy.selectedText?.isEmpty != false else { return nil }
        proxy.insertText(replacement + delimiter)
        return transaction
    }

    static func undo(_ transaction: KeyboardCorrectionTransaction, proxy: any UITextDocumentProxy,
                     lifetime: UUID, generation: UInt64) -> Bool {
        guard proxy.selectedText?.isEmpty != false,
              transaction.permits(before: proxy.documentContextBeforeInput, after: proxy.documentContextAfterInput,
                                  document: proxy.documentIdentifier, lifetime: lifetime, generation: generation) else { return false }
        let inserted = transaction.replacement + transaction.delimiter
        for remaining in stride(from: inserted.count, through: 1, by: -1) {
            guard proxy.documentIdentifier == transaction.document, proxy.selectedText?.isEmpty != false,
                  proxy.documentContextBeforeInput?.hasSuffix(String(inserted.prefix(remaining))) == true else { return false }
            proxy.deleteBackward()
        }
        guard proxy.documentIdentifier == transaction.document, proxy.selectedText?.isEmpty != false else { return false }
        proxy.insertText(transaction.restoredText)
        return true
    }
}
