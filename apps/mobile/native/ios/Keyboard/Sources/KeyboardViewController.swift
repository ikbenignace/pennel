import PennelMobileKit
import SwiftUI
import UIKit

final class KeyboardViewController: UIInputViewController {
    private var host: UIHostingController<KeyboardRoot>?
    private var model: KeyboardModel?
    private var typing: KeyboardTypingModel?
    private var timer: Timer?
    private var observations: [HandoffSignal.Observation] = []
    private let inputModeTarget = KeyboardInputModeTarget()
    private let lifetime = UUID()
    private var generation: UInt64 = 0
    private var isVisible = false
    private var installedFullAccess = false
    private var heightConstraint: NSLayoutConstraint?
    private var contentHeight: CGFloat?

    override func viewDidLoad() {
        super.viewDidLoad()
        inputModeTarget.controller = self
        let height = view.heightAnchor.constraint(equalToConstant: Brand.Spacing.s10 * 6 + Brand.Spacing.s5)
        height.priority = .defaultHigh
        height.isActive = true
        heightConstraint = height
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        isVisible = true
        install()
        model?.refresh()
        // Darwin signals are the fast path; the timer also catches a heartbeat that simply stops (5 s) and animates the bars.
        observations = [HandoffSignal.session, .result].map { signal in
            signal.observe { [weak self] in DispatchQueue.main.async { self?.model?.refresh() } }
        }
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: 0.2, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self else { return }
                if self.hasFullAccess != self.installedFullAccess { self.install() }
                self.model?.refresh()
                self.updateHeight()
            }
        }
    }

    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        invalidateContext()
        isVisible = false
        timer?.invalidate()
        timer = nil
        observations = []
        host?.willMove(toParent: nil)
        host?.view.removeFromSuperview()
        host?.removeFromParent()
        host = nil
        typing = nil
        model = nil
        contentHeight = nil
    }

    override func textWillChange(_ textInput: (any UITextInput)?) { invalidateContext() }

    override func textDidChange(_ textInput: (any UITextInput)?) {
        invalidateContext()
        updateTypingContext()
    }

    override func selectionWillChange(_ textInput: (any UITextInput)?) { invalidateContext() }

    override func selectionDidChange(_ textInput: (any UITextInput)?) {
        invalidateContext()
        updateTypingContext()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        updateHeight()
    }

    private func install() {
        model?.contextDidChange()
        installedFullAccess = hasFullAccess
        let defaults = hasFullAccess ? KeyboardLayout.groupDefaults : UserDefaults.standard
        let language = KeyboardPreferences.interfaceLanguage(in: defaults)
        primaryLanguage = language == .dutch ? "nl-NL" : "en-US"
        let copy = KeyboardCopy(language: language)
        let typing = KeyboardTypingModel(copy: copy, suggestionsEnabled: KeyboardPreferences.suggestionsEnabled(in: defaults))
        self.typing = typing
        if hasFullAccess {
            KeyboardPreferences.mirrorFeedback(from: defaults, to: UserDefaults.standard)
            KeyboardPreferences.setSuggestionsEnabled(typing.suggestionsEnabled, in: UserDefaults.standard)
            KeyboardPreferences.publish(dictionary: [], interfaceLanguage: language.rawValue, in: UserDefaults.standard)
        }
        let directory = hasFullAccess ? HandoffStore.groupDirectory : nil
        let store = HandoffStore(directory: directory ?? FileManager.default.temporaryDirectory.appendingPathComponent("pennel-no-group"))
        let model = KeyboardModel(
            hasFullAccess: hasFullAccess,
            client: HandoffClient(store: store),
            context: { [weak self] in self?.insertionContext }, copy: copy,
            onFeedback: { [weak self] in self?.playFeedback($0) },
            onResult: { [weak self] in self?.insert($0, typingFeedback: false) })
        self.model = model
        let layout = KeyboardLayout.stored(in: defaults)
        if hasFullAccess { layout.store(in: UserDefaults.standard) }
        if hasFullAccess { KeyboardFullAccess.markSeen(in: KeyboardLayout.groupDefaults) }
        let root = KeyboardRoot(
            model: model, typing: typing, layout: layout, showsGlobe: needsInputModeSwitchKey,
            inputModeTarget: inputModeTarget,
            openSettings: { [weak self] in
                guard let self else { return }
                AppOpener.openSettings(from: extensionContext)
            },
            insert: { [weak self] in self?.insert($0) },
            delete: { [weak self] in self?.delete() },
            space: { [weak self] in self?.insertSpace() },
            insertSuggestion: { [weak self] in self?.insertSuggestion($0) },
            toggleSuggestions: { [weak self] in self?.toggleSuggestions() },
            feedback: { [weak self] in self?.playFeedback($0) },
            heightChanged: { [weak self] height in
                guard let self else { return }
                self.contentHeight = height
                self.updateHeight()
            })
        updateTypingContext()
        loadSuggestions()
        if let host {
            host.rootView = root
            updateHeight()
            return
        }
        let host = UIHostingController(rootView: root)
        addChild(host)
        host.view.translatesAutoresizingMaskIntoConstraints = false
        host.view.backgroundColor = .clear
        view.addSubview(host.view)
        NSLayoutConstraint.activate([
            host.view.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            host.view.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            host.view.topAnchor.constraint(equalTo: view.topAnchor),
            host.view.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
        host.didMove(toParent: self)
        self.host = host
    }

    private var insertionContext: KeyboardInsertionContext? {
        guard isVisible else { return nil }
        return KeyboardInsertionContext(document: textDocumentProxy.documentIdentifier, lifetime: lifetime, generation: generation)
    }

    private func invalidateContext() {
        generation &+= 1
        model?.contextDidChange()
    }

    private func insert(_ text: String, typingFeedback: Bool = true) {
        guard isVisible else { return }
        invalidateContext()
        typing?.state.insertedCharacter()
        textDocumentProxy.insertText(text)
        if typingFeedback { playFeedback(.keyPress) }
        updateTypingContext()
    }

    private func delete() {
        guard isVisible else { return }
        invalidateContext()
        typing?.state.resetSpacing()
        textDocumentProxy.deleteBackward()
        updateTypingContext()
    }

    private func insertSpace() {
        guard isVisible, let typing else { return }
        invalidateContext()
        let edit = typing.state.spaceEdit(before: textDocumentProxy.documentContextBeforeInput, at: Date(), allowsPeriod: typing.allowsPeriod)
        switch edit {
        case .insert(let text): textDocumentProxy.insertText(text)
        case .replacePreviousCharacter(let text):
            textDocumentProxy.deleteBackward()
            textDocumentProxy.insertText(text)
        }
        playFeedback(.keyPress)
        updateTypingContext()
    }

    private func insertSuggestion(_ suggestion: String) {
        guard isVisible, let typing, typing.suggestions.contains(suggestion) else { return }
        let prefix = String((textDocumentProxy.documentContextBeforeInput ?? "").suffix(64).reversed()
            .prefix { $0.isLetter || $0.isNumber || $0 == "'" }.reversed())
        guard prefix == typing.currentPrefix,
              textDocumentProxy.documentContextAfterInput?.first.map({ !$0.isLetter && !$0.isNumber }) ?? true
        else { updateTypingContext(); return }
        invalidateContext()
        for _ in prefix { textDocumentProxy.deleteBackward() }
        typing.state.insertedCharacter()
        textDocumentProxy.insertText(suggestion + " ")
        playFeedback(.keyPress)
        updateTypingContext()
    }

    private func toggleSuggestions() {
        guard let typing else { return }
        typing.suggestionsEnabled.toggle()
        playFeedback(.selection)
        let defaults = hasFullAccess ? KeyboardLayout.groupDefaults : UserDefaults.standard
        KeyboardPreferences.setSuggestionsEnabled(typing.suggestionsEnabled, in: defaults)
        KeyboardPreferences.setSuggestionsEnabled(typing.suggestionsEnabled, in: UserDefaults.standard)
        loadSuggestions()
        updateTypingContext()
        updateHeight()
    }

    private func loadSuggestions() {
        guard let typing else { return }
        guard typing.suggestionsEnabled else {
            typing.setLexicon([], words: [])
            return
        }
        let words = hasFullAccess ? KeyboardPreferences.dictionary(in: KeyboardLayout.groupDefaults) : []
        requestSupplementaryLexicon(completion: KeyboardLexiconCompletion.make { [weak self, weak typing] entries in
            guard let self, self.isVisible, let typing, self.typing === typing, typing.suggestionsEnabled else { return }
            typing.setLexicon(entries, words: words)
            self.updateTypingContext()
        })
    }

    private func updateTypingContext() {
        guard let typing else { return }
        let kind = textDocumentProxy.keyboardType ?? .default
        typing.showsAt = kind == .emailAddress
        typing.showsURL = kind == .URL
        typing.returnEnabled = textDocumentProxy.enablesReturnKeyAutomatically != true || textDocumentProxy.hasText
        typing.allowsPeriod = ![UIKeyboardType.emailAddress, .URL, .numberPad, .decimalPad, .phonePad, .asciiCapableNumberPad].contains(kind)
        let capitalization: KeyboardCapitalization
        switch textDocumentProxy.autocapitalizationType ?? .sentences {
        case .none: capitalization = .none
        case .words: capitalization = .words
        case .allCharacters: capitalization = .allCharacters
        case .sentences: capitalization = .sentences
        @unknown default: capitalization = .none
        }
        switch textDocumentProxy.returnKeyType ?? .default {
        case .go: typing.returnKey = .go
        case .search, .google, .yahoo: typing.returnKey = .search
        case .send: typing.returnKey = .send
        case .next: typing.returnKey = .next
        case .done: typing.returnKey = .done
        case .join: typing.returnKey = .join
        case .route: typing.returnKey = .route
        case .continue: typing.returnKey = .continueKey
        default: typing.returnKey = .returnKey
        }
        if [.numberPad, .decimalPad, .asciiCapableNumberPad, .numbersAndPunctuation].contains(kind), typing.state.layer == .letters {
            typing.state.layer = .numbers
        }
        typing.update(before: textDocumentProxy.documentContextBeforeInput, capitalization: capitalization)
        if textDocumentProxy.documentContextAfterInput?.first.map({ $0.isLetter || $0.isNumber }) == true { typing.suggestions = [] }
    }

    private func updateHeight() {
        let row = max(Brand.Spacing.s10 + Brand.Spacing.s1,
                      min(UIFontMetrics.default.scaledValue(for: Brand.Spacing.s10 + Brand.Spacing.s1), Brand.Spacing.s10 + Brand.Spacing.s8))
        let suggestions = typing?.suggestionsEnabled == true ? Brand.Spacing.s10 + Brand.Spacing.s1 : 0
        let recovery = model?.lastResult == nil ? 0 : Brand.Spacing.s5
        let desired = contentHeight ?? (row * 5 + Brand.Spacing.s2 * 4 + suggestions + recovery)
        if abs((heightConstraint?.constant ?? 0) - desired) > 0.5 { heightConstraint?.constant = desired }
    }

    private func playFeedback(_ event: FeedbackEvent) {
        guard isVisible else { return }
        HapticFeedback.play(event, in: hasFullAccess ? KeyboardLayout.groupDefaults : UserDefaults.standard)
    }
}
