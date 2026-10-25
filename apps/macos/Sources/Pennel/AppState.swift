import Foundation
@preconcurrency import PennelCore
import Observation
import Speech

@MainActor
@Observable
final class AppState {
    let engine: PennelEngine
    let downloads: ModelDownloads
    let workspace = WorkspaceState()
    let preview: Bool
    var now = Date()
    @ObservationIgnored weak var dictation: (any OwnedDictationManaging)?
    @ObservationIgnored var onReplayOnboarding: (() -> Void)?
    @ObservationIgnored var onResumeOnboarding: (() -> Void)?
    @ObservationIgnored var onCheckUpdates: (() -> Void)?
    var settings: Settings
    var lastError: String?
    var globalHotkeyActive = false
    /// Set by a chat source; the meeting detail view consumes it once its transcript is loaded.
    var transcriptJump: TranscriptJump?
    private var chats: [String: MeetingChat] = [:]
    var onHotkeyChange: ((HotkeyTrigger) -> Void)?
    var onRecordingSettingsChange: (() -> Void)?
    /// Keychain is not observable; bumping this re-renders views that show key status.
    private var keyRevision = 0

    #if DEBUG && PENNEL_REFERENCE_SNAPSHOT
    init(snapshotDataDir: URL) throws {
        preview = true
        globalHotkeyActive = true
        engine = try PennelEngine.newPreview(dataDir: snapshotDataDir.path)
        downloads = ModelDownloads(engine: engine)
        settings = engine.settings()
        workspace.initialSetup = settings.onboarding.status == .pending
        workspace.attachPad(text: engine.dictationPadText()) { [engine] in try engine.saveDictationPad(text: $0) }
        workspace.recentMeetings = Array(engine.meetings().prefix(3))
        downloads.onModelsChanged = { [weak self] in
            guard let self else { return }
            self.settings = self.engine.settings()
        }
    }
    #else
    init() throws {
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        preview = Bundle.main.bundleIdentifier?.hasSuffix(".preview") == true
        if !preview { LegacyMigration.moveDataFolder(in: support) }
        let dataDir = support.appendingPathComponent(preview ? "Pennel Native Preview" : "Pennel")
        engine = try preview ? PennelEngine.newPreview(dataDir: dataDir.path) : PennelEngine(dataDir: dataDir.path)
        downloads = ModelDownloads(engine: engine)
        settings = engine.settings()
        workspace.initialSetup = settings.onboarding.status == .pending
        workspace.attachPad(text: engine.dictationPadText()) { [engine] in try engine.saveDictationPad(text: $0) }
        workspace.recentMeetings = Array(engine.meetings().prefix(3))
        downloads.onModelsChanged = { [weak self] in
            guard let self else { return }
            self.settings = self.engine.settings()
        }
        if !preview { LegacyMigration.finish(state: self, dataDir: dataDir) }
        if !preview { CliAgents.sync(into: self) }
        if #available(macOS 26, *) {
            engine.setSystemTranscriber(transcriber: AppleSpeechTranscriber())
            engine.setSystemLanguageModel(model: AppleIntelligenceModel())
            addAppleSpeechOnce()
        }
    }
    #endif

    private(set) var templates: [MeetingTemplate] = []

    func reloadTemplates() {
        templates = engine.meetingTemplates()
    }

    /// The template to use for `id`, falling back to "algemeen" when it was deleted.
    func templateId(_ id: String) -> String {
        if templates.isEmpty { reloadTemplates() }
        return templates.contains { $0.id == id } ? id : "algemeen"
    }

    /// Offers Apple's dictation model as a provider once; if the user removes it, it stays removed.
    private func addAppleSpeechOnce() {
        let key = "appleSpeechOffered"
        guard !UserDefaults.standard.bool(forKey: key) else { return }
        UserDefaults.standard.set(true, forKey: key)
        guard !settings.providers.contains(where: { $0.kind == .appleSpeech }) else { return }
        settings.providers.append(newAccount(kind: .appleSpeech, existing: settings.providers))
        save()
    }

    func chat(for meetingId: String?) -> MeetingChat {
        let key = meetingId ?? ""
        if let chat = chats[key] { return chat }
        let chat = MeetingChat(meetingId: meetingId)
        chats[key] = chat
        return chat
    }

    func save() {
        do {
            try engine.updateSettings(settings: settings)
            settings = engine.settings()
            NativeAppearance.apply(settings.appearance)
            if !settings.dictationPadAutoPaste { dictation?.cancelExternalPadDelivery() }
            lastError = nil
        } catch {
            lastError = "Instellingen bewaren mislukt: \(error)"
        }
    }

    func keys() -> ApiKeys {
        var byProvider: [String: String] = [:]
        for account in settings.providers {
            if let key = Keychain.get(account.id) { byProvider[account.id] = key }
        }
        return ApiKeys(byProvider: byProvider)
    }

    func account(_ id: String) -> ProviderAccount? {
        settings.providers.first { $0.id == id }
    }

    func hasKey(_ account: ProviderAccount) -> Bool {
        _ = keyRevision
        return Keychain.get(account.id) != nil
    }

    @discardableResult
    func setKey(_ key: String, for account: ProviderAccount) -> Bool {
        guard key != (Keychain.get(account.id) ?? "") else { return true }
        guard Keychain.set(key, for: account.id) else {
            lastError = "API-sleutel bewaren mislukt. Je bestaande sleutel is behouden."
            return false
        }
        keyRevision += 1
        return true
    }

    func isReady(_ choice: ModelChoice) -> Bool {
        guard let account = account(choice.providerId) else { return false }
        if account.kind == .local { return downloads.states[choice.model] == .downloaded }
        if account.kind == .appleIntelligence { return FoundationLanguageModel.availability().status == "available" }
        if account.kind == .appleSpeech {
            if #available(macOS 26, *) { return SFSpeechRecognizer.authorizationStatus() == .authorized }
            return false
        }
        return !requiresKey(kind: account.kind) || hasKey(account)
    }

    var needsOnboarding: Bool {
        settings.onboarding.status == .pending
    }

    var setupFacts: SetupFacts {
        _ = workspace.permissionRevision
        return SetupFacts(
            microphone: Permissions.microphone == .authorized ? .available : .permissionRequired,
            globalHotkey: Permissions.accessibilityTrusted ? (globalHotkeyActive ? .available : .unavailable) : .permissionRequired,
            externalPaste: Permissions.accessibilityTrusted ? .available : .permissionRequired,
            appleSpeech: {
                guard #available(macOS 26, *) else { return .unsupported }
                return SFSpeechRecognizer.authorizationStatus() == .authorized ? .available : .permissionRequired
            }(),
            keyedProviders: settings.providers.filter { hasKey($0) }.map(\.id)
        )
    }

    var dictationSetup: SetupReadiness {
        dictationReadiness(settings: settings, facts: setupFacts, downloadedModels: downloads.downloaded.map(\.id))
    }

    var meetingSetup: SetupReadiness {
        meetingReadiness(settings: settings, facts: setupFacts, downloadedModels: downloads.downloaded.map(\.id))
    }

    var canSummarize: Bool {
        guard let account = account(settings.llm.providerId), supports(kind: account.kind, task: .llm) else { return false }
        if isCliAgent(kind: account.kind) { return FileManager.default.isExecutableFile(atPath: account.baseUrl) }
        guard !settings.llm.model.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return false }
        return isReady(settings.llm)
    }

    func setOnboarding(step: OnboardingStep, status: OnboardingStatus? = nil) {
        settings.onboarding.resumeStep = step
        if let status { settings.onboarding.status = status }
        save()
    }

    @discardableResult
    func flushWorkspaceDrafts() -> Bool {
        let pad = workspace.flushPad()
        let notes = workspace.flushMeetingNotes { [engine] id, text in try engine.setMeetingNotes(id: id, text: text) }
        if !notes { lastError = "Notities bewaren mislukt. Je wijzigingen blijven in Pennel staan." }
        return pad && notes
    }
}
