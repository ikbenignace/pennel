import Dispatch
import PennelMobileKit
import Testing
import UIKit

@MainActor
struct KeyboardLexiconCompletionTests {
    @Test func worker_completion_reads_and_applies_lexicon_on_main_actor() async {
        let entries = await receive(FakeLexicon([("pennel", "Pennel"), ("brb", "be right back")]))
        #expect(entries.map(\.input) == ["pennel", "brb"])
        #expect(entries.map(\.replacement) == ["Pennel", "be right back"])
    }

    @Test func worker_completion_accepts_an_empty_lexicon() async {
        let entries = await receive(FakeLexicon([]))
        #expect(entries.isEmpty)
    }

    @Test func worker_completion_limits_the_lexicon_to_2048_entries() async {
        let lexicon = FakeLexicon((0..<2_049).map { ("input\($0)", "replacement\($0)") })
        let entries = await receive(lexicon)
        #expect(entries.count == 2_048)
        #expect(entries.first?.input == "input0")
        #expect(entries.last?.input == "input2047")
        #expect(entries.last?.replacement == "replacement2047")
    }

    @Test func main_actor_completion_also_applies_the_lexicon() async {
        let entries = await receive(FakeLexicon([("hello", "Hello")]), fromWorker: false)
        #expect(entries.map(\.replacement) == ["Hello"])
    }

    private func receive(_ lexicon: UILexicon, fromWorker: Bool = true) async -> [KeyboardLexiconEntry] {
        await withCheckedContinuation { continuation in
            let completion = KeyboardLexiconCompletion.make { entries in
                MainActor.preconditionIsolated()
                continuation.resume(returning: entries)
            }
            if fromWorker {
                Task.detached {
                    dispatchPrecondition(condition: .notOnQueue(.main))
                    completion(lexicon)
                }
            } else {
                completion(lexicon)
            }
        }
    }
}

@MainActor
private final class FakeLexicon: UILexicon {
    private let values: [UILexiconEntry]

    init(_ entries: [(String, String)]) {
        values = entries.map { FakeEntry(input: $0.0, replacement: $0.1) }
        super.init()
    }

    override var entries: [UILexiconEntry] {
        MainActor.preconditionIsolated()
        return values
    }
}

@MainActor
private final class FakeEntry: UILexiconEntry {
    private let input: String
    private let replacement: String

    init(input: String, replacement: String) {
        self.input = input
        self.replacement = replacement
        super.init()
    }

    override var userInput: String {
        MainActor.preconditionIsolated()
        return input
    }

    override var documentText: String {
        MainActor.preconditionIsolated()
        return replacement
    }
}
