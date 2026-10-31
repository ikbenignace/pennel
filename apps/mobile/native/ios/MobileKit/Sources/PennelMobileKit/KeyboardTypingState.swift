import Foundation

public enum KeyboardLayer: String, Sendable {
    case letters, numbers, symbols
}

public enum KeyboardShift: Sendable {
    case off, once, locked
}

public enum KeyboardCapitalization: Sendable {
    case none, words, sentences, allCharacters
}

public enum KeyboardTextEdit: Equatable, Sendable {
    case insert(String)
    case replacePreviousCharacter(String)
}

public struct KeyboardTypingState: Sendable {
    public var layer: KeyboardLayer = .letters
    public private(set) var shift: KeyboardShift = .off
    private var lastShift: Date?
    private var lastSpace: Date?
    private var manualShift = false

    public init() {}

    public mutating func pressShift(at date: Date) {
        if shift != .locked, let lastShift, date.timeIntervalSince(lastShift) <= 0.35 {
            shift = .locked
        } else {
            shift = shift == .off ? .once : .off
        }
        manualShift = true
        lastShift = date
    }

    public mutating func insertedCharacter() {
        if shift == .once { shift = .off }
        manualShift = false
        lastShift = nil
        lastSpace = nil
    }

    public mutating func updateCapitalization(before: String?, mode: KeyboardCapitalization) {
        guard shift != .locked, !manualShift else { return }
        let before = before ?? ""
        let uppercase: Bool
        switch mode {
        case .none: uppercase = false
        case .allCharacters: uppercase = true
        case .words: uppercase = before.isEmpty || before.last?.isWhitespace == true
        case .sentences:
            let trimmed = before.trimmingCharacters(in: .whitespaces)
            uppercase = trimmed.isEmpty || trimmed.last == "\n"
                || (before.last?.isWhitespace == true && trimmed.last.map { ".!?".contains($0) } == true)
        }
        shift = uppercase ? .once : .off
    }

    public mutating func spaceEdit(before: String?, at date: Date, allowsPeriod: Bool = true) -> KeyboardTextEdit {
        defer { lastSpace = date }
        guard allowsPeriod, let lastSpace, date.timeIntervalSince(lastSpace) <= 0.7,
              let before, before.last == " ", before.count >= 2,
              before.dropLast().last.map({ $0.isLetter || $0.isNumber }) == true
        else { return .insert(" ") }
        return .replacePreviousCharacter(". ")
    }

    public mutating func resetSpacing() { lastSpace = nil }

    public func text(for key: String) -> String {
        layer == .letters && shift != .off ? key.uppercased() : key
    }

    public func rows(layout: KeyboardLayout) -> [[String]] {
        switch layer {
        case .letters: layout.rows
        case .numbers: [
            Array("1234567890").map(String.init),
            ["-", "/", ":", ";", "(", ")", "€", "&", "@", "\""],
            [".", ",", "?", "!", "'"],
        ]
        case .symbols: [
            ["[", "]", "{", "}", "#", "%", "^", "*", "+", "="],
            ["_", "\\", "|", "~", "<", ">", "$", "£", "¥", "•"],
            [".", ",", "?", "!", "'"],
        ]
        }
    }
}

public struct KeyboardLexiconEntry: Sendable {
    public let input: String
    public let replacement: String

    public init(input: String, replacement: String) {
        self.input = input
        self.replacement = replacement
    }
}

public struct KeyboardSuggestionIndex: Sendable {
    private let entries: [KeyboardLexiconEntry]

    public init(words: [String], supplementary: [KeyboardLexiconEntry] = []) {
        var seen: Set<String> = []
        var bounded: [KeyboardLexiconEntry] = []
        let candidates = supplementary + words.sorted().map { KeyboardLexiconEntry(input: $0, replacement: $0) }
        for entry in candidates {
            let input = entry.input.trimmingCharacters(in: .whitespacesAndNewlines)
            let replacement = entry.replacement.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !input.isEmpty, input.count <= 64, !replacement.isEmpty, replacement.count <= 128,
                  seen.insert(input.lowercased()).inserted else { continue }
            bounded.append(KeyboardLexiconEntry(input: input, replacement: replacement))
            if bounded.count == 2_048 { break }
        }
        entries = bounded.sorted { $0.input.localizedStandardCompare($1.input) == .orderedAscending }
    }

    public func suggestions(for prefix: String, limit: Int = 3) -> [String] {
        guard (2...64).contains(prefix.count), limit > 0 else { return [] }
        let folded = prefix.lowercased()
        return Array(entries.lazy.filter { $0.input.lowercased().hasPrefix(folded) }
            .map { entry in
                if prefix == prefix.uppercased() { return entry.replacement.uppercased() }
                if prefix.first?.isUppercase == true {
                    return entry.replacement.prefix(1).uppercased() + entry.replacement.dropFirst()
                }
                return entry.replacement
            }.prefix(min(limit, 3)))
    }
}
