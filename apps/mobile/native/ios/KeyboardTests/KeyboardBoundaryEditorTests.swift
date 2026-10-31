import Foundation
import PennelMobileKit
import Testing
import UIKit

@MainActor
private final class BoundaryTestDocument: NSObject, UITextDocumentProxy {
    var text: String
    var cursor: Int
    var followingContextIsUnavailable: Bool
    var beforeContextIsUnavailable = false
    var selectedText: String?
    var documentIdentifier = UUID()
    var documentInputMode: UITextInputMode? { nil }
    var deletionCount = 0

    init(_ text: String, followingContextIsUnavailable: Bool = true) {
        self.text = text
        cursor = text.count
        self.followingContextIsUnavailable = followingContextIsUnavailable
    }

    var documentContextBeforeInput: String? {
        beforeContextIsUnavailable ? nil : String(text.prefix(cursor))
    }

    var documentContextAfterInput: String? {
        followingContextIsUnavailable ? nil : String(text.dropFirst(cursor))
    }

    var hasText: Bool { !text.isEmpty }

    func insertText(_ text: String) {
        let position = self.text.index(self.text.startIndex, offsetBy: cursor)
        self.text.insert(contentsOf: text, at: position)
        cursor += text.count
    }

    func deleteBackward() {
        guard cursor > 0 else { return }
        let position = text.index(text.startIndex, offsetBy: cursor - 1)
        text.remove(at: position)
        cursor -= 1
        deletionCount += 1
    }

    func adjustTextPosition(byCharacterOffset offset: Int) { cursor = max(0, min(text.count, cursor + offset)) }
    func setMarkedText(_ markedText: String, selectedRange: NSRange) {}
    func unmarkText() {}
}

@MainActor
struct KeyboardBoundaryEditorTests {
    @Test func actual_controller_edit_path_corrects_and_undoes_with_nil_after_context() throws {
        let proxy = BoundaryTestDocument("Teh teh")
        let lifetime = UUID()
        let transaction = try #require(KeyboardBoundaryEditor.apply(original: "teh", replacement: "the", delimiter: " ",
            proxy: proxy, lifetime: lifetime))
        #expect(proxy.text == "Teh the ")
        #expect(proxy.deletionCount == 3)
        let bound = try #require(transaction.bound(to: KeyboardInsertionContext(
            document: proxy.documentIdentifier, lifetime: lifetime, generation: 9)))
        #expect(KeyboardBoundaryEditor.undo(bound, proxy: proxy, lifetime: lifetime, generation: 9))
        #expect(proxy.text == "Teh teh ")
    }

    @Test func nil_after_is_not_assumed_to_mean_eof_and_following_text_is_not_deleted() throws {
        let proxy = BoundaryTestDocument("teh following")
        proxy.cursor = 3
        let transaction = try #require(KeyboardBoundaryEditor.apply(original: "teh", replacement: "the", delimiter: " ",
            proxy: proxy, lifetime: UUID()))
        #expect(proxy.text == "the  following")
        #expect(transaction.original == "teh")
        #expect(proxy.deletionCount == 3)
    }

    @Test func known_following_letters_selection_and_unproven_prefixes_fail_without_deletion() {
        let proxy = BoundaryTestDocument("tehnical", followingContextIsUnavailable: false)
        proxy.cursor = 3
        #expect(KeyboardBoundaryEditor.apply(original: "teh", replacement: "the", delimiter: " ", proxy: proxy, lifetime: UUID()) == nil)
        #expect(proxy.text == "tehnical")
        proxy.followingContextIsUnavailable = true
        proxy.cursor = proxy.text.count
        #expect(KeyboardBoundaryEditor.apply(original: "teh", replacement: "the", delimiter: " ", proxy: proxy, lifetime: UUID()) == nil)
        proxy.text = "teh"
        proxy.cursor = 3
        proxy.selectedText = "teh"
        #expect(KeyboardBoundaryEditor.apply(original: "teh", replacement: "the", delimiter: " ", proxy: proxy, lifetime: UUID()) == nil)
        proxy.selectedText = nil
        proxy.beforeContextIsUnavailable = true
        #expect(KeyboardBoundaryEditor.apply(original: "teh", replacement: "the", delimiter: " ", proxy: proxy, lifetime: UUID()) == nil)
        #expect(proxy.deletionCount == 0)
    }

    @Test func undo_rejects_generation_document_and_context_availability_changes() throws {
        let proxy = BoundaryTestDocument("teh")
        let lifetime = UUID()
        let transaction = try #require(KeyboardBoundaryEditor.apply(original: "teh", replacement: "the", delimiter: " ",
            proxy: proxy, lifetime: lifetime))
        let bound = try #require(transaction.bound(to: KeyboardInsertionContext(
            document: proxy.documentIdentifier, lifetime: lifetime, generation: 7)))
        let deletions = proxy.deletionCount
        #expect(!KeyboardBoundaryEditor.undo(bound, proxy: proxy, lifetime: lifetime, generation: 8))
        proxy.followingContextIsUnavailable = false
        #expect(!KeyboardBoundaryEditor.undo(bound, proxy: proxy, lifetime: lifetime, generation: 7))
        proxy.followingContextIsUnavailable = true
        proxy.documentIdentifier = UUID()
        #expect(!KeyboardBoundaryEditor.undo(bound, proxy: proxy, lifetime: lifetime, generation: 7))
        #expect(proxy.deletionCount == deletions)
        #expect(proxy.text == "the ")
    }
}
