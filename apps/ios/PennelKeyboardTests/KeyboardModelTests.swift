import Foundation
import PennelMobileKit
import Testing

@MainActor
struct KeyboardExtensionModelTests {
    @Test func extension_uses_the_shared_tested_session_model() {
        let store = HandoffStore(directory: FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString))
        let context = KeyboardInsertionContext(document: UUID(), lifetime: UUID(), generation: 0)
        let model = KeyboardModel(
            hasFullAccess: false, client: HandoffClient(store: store), context: { context }, onResult: { _ in })
        model.refresh()
        #expect(model.pad == .needsFullAccess)
        model.tapPill()
        #expect(store.read(HandoffCommand.self, as: .command) == nil)
    }

    @Test func fallback_does_not_claim_a_ready_microphone() {
        let store = HandoffStore(directory: FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString))
        let context = KeyboardInsertionContext(document: UUID(), lifetime: UUID(), generation: 0)
        let model = KeyboardModel(
            hasFullAccess: true, client: HandoffClient(store: store), context: { context }, onResult: { _ in })
        model.refresh()
        model.tapPill()
        #expect(model.pad == .sessionOff)
        #expect(store.read(HandoffCommand.self, as: .command) == nil)
    }
}
