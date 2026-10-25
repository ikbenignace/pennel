#if DEBUG && PENNEL_REFERENCE_SNAPSHOT
import AppKit
import AVFoundation
import CoreText
import Observation
@preconcurrency import PennelCore
import PennelKit
import Speech
import SQLite3
import SwiftUI

/// Compiled only by capture-native-reference.sh with isolated data and inert services.
@main
@MainActor
final class WindowSnapshot: NSObject, NSApplicationDelegate {
    private static let meetingId = "2026-10-24 0900 Projectoverleg"
    private static let title = "Projectoverleg"
    private static let start = "2026-10-24T09:00:00+02:00"
    private static let now = ISO8601DateFormatter().date(from: "2026-10-25T09:00:00Z")!
    private static let sentence = "Zullen we morgen om tien uur afspreken?"
    private static let notes = "- Planning afronden\n- Volgende afspraak bevestigen"
    private static let summary = "## Besluiten\n- De planning wordt maandag afgerond.\n\n## Actiepunten\n- [ ] Volgende afspraak bevestigen."
    private var states: [AppState] = []
    private var managers: [SnapshotDictation] = []

    static func main() {
        UserDefaults.standard.register(defaults: ["AppleLanguages": ["nl"], "AppleLocale": "nl_BE"])
        NSTimeZone.default = TimeZone(identifier: "Europe/Brussels")!
        let app = NSApplication.shared
        let delegate = WindowSnapshot()
        app.delegate = delegate
        app.setActivationPolicy(.prohibited)
        app.run()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        Task { @MainActor in
            do { try await run(); NSApp.terminate(nil) }
            catch {
                FileHandle.standardError.write(Data("Native snapshot failed: \(error)\n".utf8))
                exit(1)
            }
        }
    }

    private func run() async throws {
        guard CommandLine.arguments.count == 3 else { throw SnapshotError.arguments }
        let output = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true).standardizedFileURL
        let fixture = URL(fileURLWithPath: CommandLine.arguments[2], isDirectory: true).resolvingSymlinksInPath()
        let temporary = FileManager.default.temporaryDirectory.resolvingSymlinksInPath()
        guard fixture.deletingLastPathComponent() == temporary,
              fixture.lastPathComponent.hasPrefix("pennel-native-reference-") else { throw SnapshotError.fixture }
        try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
        registerFonts()
        guard let registry = Bundle.main.url(forResource: "visual-cases", withExtension: "json"),
              let definitions = try JSONSerialization.jsonObject(with: Data(contentsOf: registry)) as? [[String: Any]] else { throw SnapshotError.fixture }
        let registered = definitions.compactMap { $0["name"] as? String }
        guard registered.count == definitions.count, Set(registered).count == registered.count else { throw SnapshotError.fixture }
        let byName = Dictionary(uniqueKeysWithValues: definitions.map { ($0["name"] as! String, $0) })
        let requested = ProcessInfo.processInfo.environment["PENNEL_REFERENCE_CASES"]?.split(separator: ",").map(String.init) ?? []
        guard requested.allSatisfy(registered.contains) else { throw SnapshotError.arguments }
        let cases = requested.isEmpty ? registered : registered.filter(requested.contains)
        let appearance = ProcessInfo.processInfo.environment["PENNEL_REFERENCE_APPEARANCE"] ?? "both"
        guard ["light", "dark", "both"].contains(appearance) else { throw SnapshotError.arguments }
        let appearances: [ColorScheme] = appearance == "both" ? [.light, .dark] : [appearance == "dark" ? .dark : .light]
        var images: [[String: Any]] = []
        for scheme in appearances {
            let name = scheme == .dark ? "dark" : "light"
            let themeOutput = output.appendingPathComponent(name, isDirectory: true)
            try FileManager.default.createDirectory(at: themeOutput, withIntermediateDirectories: true)
            for item in cases {
                let definition = byName[item]!
                if definition["window"] as? String == "pill" {
                    let model = PillModel()
                    model.show(definition["pill_state"] as? String == "working" ? .working : .listening)
                    model.preview.begin("qa-" + item)
                    if let text = definition["preview_text"] as? String { model.preview.update(sessionID: "qa-" + item, revision: 7, text: text) }
                    let view = PillView(model: model).frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottom)
                    images.append(try await capture(item, view: view, size: NSSize(width: ThemePill.windowWidth, height: ThemePill.windowHeight), scheme: scheme, output: themeOutput, transparent: true))
                    continue
                }
                let data = fixture.appendingPathComponent(name + "-" + item, isDirectory: true)
                let state = try AppState(snapshotDataDir: data)
                state.now = Self.now
                state.settings.appearance = scheme == .dark ? .dark : .light
                state.settings.providers = [
                    ProviderAccount(id: "local", kind: .local, name: "Lokaal", baseUrl: ""),
                    ProviderAccount(id: "groq", kind: .groq, name: "Groq", baseUrl: "https://api.groq.com/openai/v1"),
                    ProviderAccount(id: "apple-speech", kind: .appleSpeech, name: "Apple-dicteren", baseUrl: ""),
                    ProviderAccount(id: "openai", kind: .openAi, name: "OpenAI", baseUrl: "https://api.openai.com/v1"),
                ]
                state.settings.dictationStt = ModelChoice(providerId: "local", model: "whisper-large-v3-turbo")
                state.settings.meetingStt = ModelChoice(providerId: "local", model: "whisper-large-v3-turbo")
                state.settings.llm = ModelChoice(providerId: "groq", model: "llama-3.3-70b-versatile")
                state.settings.historyRetentionDays = 30
                state.settings.meetingAudioRetentionDays = 30
                state.settings.styleRules = [StyleRule(bundleId: "com.apple.mail", style: .formal)]
                state.settings.language = "nl"
                state.settings.dictionary = ["Pennel", "Studio Noord"]
                state.workspace.padText = Self.sentence
                state.settings.onboarding = OnboardingProgress(version: 1, status: .completed, resumeStep: .welcome)
                Keychain.accounts = []
                Permissions.microphone = .authorized
                Permissions.accessibilityTrusted = true
                SFSpeechRecognizer.status = .authorized
                let manager = SnapshotDictation(state: state, caseName: item)
                state.dictation = manager
                states.append(state)
                managers.append(manager)
                if item != "meetings-empty" { try seedMeeting(state) }
                if item == "history" { try seedHistory(data.appendingPathComponent("history.sqlite")) }
                let meeting = MeetingController(state: state, recording: item == "recording", meetingId: item == "recording" ? Self.meetingId : nil)
                configure(state, for: item)
                let onboarding = item.hasPrefix("onboarding-")
                let view: AnyView = onboarding ? AnyView(OnboardingView(state: state))
                    : AnyView(MainWorkspaceView(state: state, meeting: meeting, calendar: CalendarProvider()))
                let size = onboarding ? NSSize(width: 850, height: 690) : NSSize(width: 1040, height: 740)
                images.append(try await capture(item, view: view, size: size, scheme: scheme, output: themeOutput))
            }
        }
        let manifest: [String: Any] = [
            "capture_method": "NSHostingView.cacheDisplay on owned offscreen windows; no display capture",
            "platform_services": "inert QA replacements; no Keychain, permission requests, calendar, audio, CLI, or provider calls",
            "presentation_sources": "Production MainWorkspaceView, OnboardingView and PillView with isolated fixture models",
            "fixture_clock": "2026-10-25T09:00:00Z", "images": images,
            "fixture": ["meeting_id": Self.meetingId, "title": Self.title, "start_iso": Self.start, "duration_min": 25,
                        "notes": Self.notes, "summary": Self.summary, "pad": Self.sentence, "live_elapsed_secs": 754] as [String: Any],
        ]
        try JSONSerialization.data(withJSONObject: manifest, options: [.prettyPrinted, .sortedKeys])
            .write(to: output.appendingPathComponent("fixture.json"), options: .atomic)
    }

    private func configure(_ state: AppState, for name: String) {
        state.workspace.recentMeetings = name == "meetings-empty" ? [] : state.engine.meetings()
        state.downloads.configureSnapshot(states: [state.settings.dictationStt.model: .downloaded], progress: [:], errors: [:])
        switch name {
        case "dictation-pad": state.workspace.dictationPadOpen = true
        case "dictation-shortcut-denied", "dictation-pad-shortcut-denied":
            state.workspace.destination = .dictation
            state.workspace.dictationPadOpen = name == "dictation-pad-shortcut-denied"
            state.settings.onboarding.status = .skipped
            Permissions.accessibilityTrusted = false
        case "meetings-empty": state.workspace.destination = .meetings
        case "meeting-notes", "meeting-transcript", "meeting-questions":
            state.workspace.selectMeeting(Self.meetingId)
            state.workspace.meetingTabs[Self.meetingId] = name == "meeting-transcript" ? .transcript : name == "meeting-questions" ? .ask : .notes
        case "recording": state.workspace.destination = .meetings; state.workspace.showingRecording = true
        case "history": state.workspace.destination = .history
        case let value where value.hasPrefix("settings-"):
            state.workspace.destination = .settings
            switch value {
            case "settings-models": state.workspace.settingsTab = .models
            case "settings-dictation": state.workspace.settingsTab = .dictation
            case "settings-meetings": state.workspace.settingsTab = .meetings
            case "settings-recording": state.workspace.settingsTab = .recording
            case "settings-dictionary": state.workspace.settingsTab = .dictionary
            case "settings-styles": state.workspace.settingsTab = .styles
            case "settings-appearance": state.workspace.settingsTab = .appearance
            default: state.workspace.settingsTab = .providers
            }
        case let value where value.hasPrefix("onboarding-"):
            let step: OnboardingStep = value == "onboarding-welcome" ? .welcome
                : value.contains("permissions") || value == "onboarding-permission-denied" ? .permissions
                : value.contains("practice") ? .practice : .engine
            state.settings.onboarding = OnboardingProgress(version: 1, status: .pending, resumeStep: step)
            state.workspace.onboardingInitialized = true
            state.workspace.onboardingEngine = value == "onboarding-engine-apple" ? "apple" : value == "onboarding-engine-custom" ? "custom" : "local"
            state.workspace.onboardingModel = state.settings.dictationStt.model
            state.workspace.onboardingProvider = state.settings.dictationStt.providerId
            if step == .engine { state.downloads.configureSnapshot(states: [state.settings.dictationStt.model: .notDownloaded], progress: [:], errors: [:]) }
            if value == "onboarding-engine-apple" || value == "onboarding-engine-custom" {
                let account = state.settings.providers.first { $0.kind == (value == "onboarding-engine-apple" ? .appleSpeech : .openAi) }!
                let model = value == "onboarding-engine-apple" ? "dictation" : "gpt-4o-mini-transcribe"
                state.settings.dictationStt = ModelChoice(providerId: account.id, model: model)
                state.workspace.onboardingProvider = account.id
                state.workspace.onboardingModel = model
            }
            if value == "onboarding-permissions" { Permissions.microphone = .notDetermined; Permissions.accessibilityTrusted = false }
            if value == "onboarding-permission-denied" { Permissions.microphone = .denied; Permissions.accessibilityTrusted = false }
            if value == "onboarding-download-error" {
                state.downloads.configureSnapshot(states: [state.settings.dictationStt.model: .notDownloaded], progress: [:], errors: [state.settings.dictationStt.model: "Het model kon niet worden gedownload. Controleer je verbinding en probeer opnieuw."])
            }
        default: state.workspace.destination = .dictation
        }
    }

    private func seedMeeting(_ state: AppState) throws {
        let folder = URL(fileURLWithPath: state.settings.meetingsDir, isDirectory: true).appendingPathComponent(Self.meetingId, isDirectory: true)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        let markdown = "---\ntitle: \(Self.title)\nstart: \(Self.start)\nduration_min: 25\ntemplate: algemeen\nspeakers: 1=Noor\nattendees: [Noor, Sam]\n---\n\n\(Self.summary)\n"
        let transcript = "[00:00] **Anderen 1:** We ronden de planning maandag af.\n\n[00:08] **Ik:** Ik bevestig de volgende afspraak.\n"
        try markdown.write(to: folder.appendingPathComponent("meeting.md"), atomically: true, encoding: .utf8)
        try Self.notes.write(to: folder.appendingPathComponent("notes.md"), atomically: true, encoding: .utf8)
        try transcript.write(to: folder.appendingPathComponent("transcript.md"), atomically: true, encoding: .utf8)
    }

    private func seedHistory(_ path: URL) throws {
        var connection: OpaquePointer?
        guard sqlite3_open(path.path, &connection) == SQLITE_OK else { throw SnapshotError.fixture }
        defer { sqlite3_close(connection) }
        let sql = "INSERT INTO dictations VALUES ('qa-dictation-2',1792917900000,'dev.pennel.Pennel.preview','\(Self.sentence)','\(Self.sentence)','done',NULL,0); INSERT INTO dictations VALUES ('qa-dictation-1',1792915200000,'com.apple.mail','ik werk het voorstel bij en stuur het morgen door','Ik werk het voorstel bij en stuur het morgen door.','done',NULL,0);"
        guard sqlite3_exec(connection, sql, nil, nil, nil) == SQLITE_OK else { throw SnapshotError.fixture }
    }

    private func registerFonts() {
        guard let resources = Bundle.main.resourceURL,
              let fonts = try? FileManager.default.contentsOfDirectory(at: resources.appendingPathComponent("Fonts"), includingPropertiesForKeys: nil) else { return }
        for font in fonts where font.pathExtension == "ttf" { CTFontManagerRegisterFontsForURL(font as CFURL, .process, nil) }
    }

    private func capture(_ name: String, view: some View, size: NSSize, scheme: ColorScheme, output: URL, transparent: Bool = false) async throws -> [String: Any] {
        let window = SnapshotWindow(contentRect: NSRect(origin: NSPoint(x: -10000, y: -10000), size: size), styleMask: .borderless, backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.appearance = NSAppearance(named: scheme == .dark ? .darkAqua : .aqua)
        NSApp.appearance = window.appearance
        window.backgroundColor = transparent ? .clear : NSColor(Theme.paper)
        window.isOpaque = !transparent
        let host = NSHostingView(rootView: view.font(.figtree(13)).foregroundStyle(Theme.ink).environment(\.colorScheme, scheme).environment(\.locale, Locale(identifier: "nl_BE")))
        host.frame = NSRect(origin: .zero, size: size)
        window.contentView = host
        window.orderFront(nil)
        host.layoutSubtreeIfNeeded()
        try await Task.sleep(for: .milliseconds(400))
        host.layoutSubtreeIfNeeded()
        window.displayIfNeeded()
        guard let bitmap = host.bitmapImageRepForCachingDisplay(in: host.bounds) else { throw SnapshotError.bitmap }
        host.cacheDisplay(in: host.bounds, to: bitmap)
        guard let png = bitmap.representation(using: .png, properties: [:]) else { throw SnapshotError.bitmap }
        try png.write(to: output.appendingPathComponent(name + ".png"), options: .atomic)
        window.orderOut(nil)
        window.contentView = nil
        window.close()
        return ["case": name, "appearance": scheme == .dark ? "dark" : "light", "file": (scheme == .dark ? "dark/" : "light/") + name + ".png", "logical_width": size.width, "logical_height": size.height, "pixel_width": bitmap.pixelsWide, "pixel_height": bitmap.pixelsHigh]
    }
}

private enum SnapshotError: Error { case arguments, fixture, bitmap, platformService }

@MainActor
private final class SnapshotWindow: NSWindow {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
    override func constrainFrameRect(_ frameRect: NSRect, to screen: NSScreen?) -> NSRect { frameRect }
}

@MainActor
private final class SnapshotDictation: OwnedDictationManaging {
    private weak var state: AppState?
    private let caseName: String
    init(state: AppState, caseName: String) { self.state = state; self.caseName = caseName }
    func openSession(kind: OwnedDictationKind) -> OwnedDictationSession? {
        if let current = state?.workspace.session, current.kind == kind { return current }
        let session = OwnedDictationSession(id: "qa-" + caseName, kind: kind)
        session.text = kind == .pad ? state?.workspace.padText ?? "" : caseName == "onboarding-practice" ? "Zullen we morgen om tien uur afspreken?" : ""
        if caseName == "onboarding-practice" { session.phase = .done }
        if caseName == "onboarding-practice-error" { session.phase = .error; session.error = "Geen spraak herkend. Controleer je microfoon en probeer opnieuw." }
        state?.workspace.session = session
        return session
    }
    func toggleSession(_ id: String) {}
    func beginSession(_ id: String) {}
    func submitSession(_ id: String) {}
    func cancelSession(_ id: String) {}
    func closeSession(_ id: String) {}
    func cancelExternalPadDelivery() {}
    func shutdownOwnedSession() {}
}

@MainActor
enum SFSpeechRecognizer {
    static var status: SFSpeechRecognizerAuthorizationStatus = .authorized
    static func authorizationStatus() -> SFSpeechRecognizerAuthorizationStatus { status }
    static func requestAuthorization(_ callback: @escaping (SFSpeechRecognizerAuthorizationStatus) -> Void) { callback(status) }
}
struct NativeLanguageModelMessage { let role: String; let content: String }
struct NativeLanguageModelAvailability { let status: String; let reason: String? }
enum FoundationLanguageModel {
    static func availability() -> NativeLanguageModelAvailability { .init(status: "available", reason: nil) }
    static func complete(messages: [NativeLanguageModelMessage], timeoutSecs: UInt64) async throws -> String { throw SnapshotError.platformService }
}
@MainActor
enum NativeAppearance {
    static func apply(_ appearance: AppearancePreference) {
        switch appearance {
        case .light: NSApp.appearance = NSAppearance(named: .aqua)
        case .dark: NSApp.appearance = NSAppearance(named: .darkAqua)
        case .system: NSApp.appearance = nil
        }
    }
}

@MainActor
enum Keychain {
    static var accounts = Set<String>()
    static func get(_ account: String) -> String? { accounts.contains(account) ? "fixture-only-key" : nil }
    static func set(_ value: String?, for account: String) -> Bool {
        if value?.isEmpty == false { accounts.insert(account) } else { accounts.remove(account) }
        return true
    }
}

extension HotkeyTrigger {
    var label: String {
        switch self {
        case .fn: "fn"
        case .controlSuper: "Ctrl + Super"
        case .rightOption: "rechter Option"
        case .rightCommand: "rechter Command"
        case .rightControl: "rechter Ctrl"
        }
    }
}

@MainActor
enum Permissions {
    static var microphone: AVAuthorizationStatus = .authorized
    static var accessibilityTrusted = true
    static func requestMicrophone() async -> Bool { false }
    static func promptAccessibility() {}
}

@MainActor
final class CalendarProvider {
    var hasAccess: Bool { false }
    func requestAccess() async -> Bool { false }
    func calendars() -> [(id: String, name: String)] { [] }
}

@MainActor
enum CliAgents { static func dismiss(_ kind: ProviderKind) {} }

struct Diagnostics: Sendable {
    init(_ category: String) {}
    func error(_ message: String) {}
    func info(_ message: String) {}
    func notice(_ message: String) {}
    func debug(_ message: String) {}
}

@MainActor
@Observable
final class MeetingController {
    enum Phase: Equatable { case idle, recording, summarizing, finished }
    var phase: Phase
    var meetingId: String?
    var elapsed: TimeInterval = 754
    var liveTranscript: [Segment] = []
    var systemAudioMissing = false
    var willSummarize = false
    var error: String?
    var detectedApp: String?
    var consentCopied = false
    static let consentSentence = "Ik neem notities met transcript via Pennel."
    var title = "Projectoverleg"
    var notes = "- Planning afronden\n- Volgende afspraak bevestigen"
    init(state: AppState, recording: Bool = false, meetingId: String? = nil) {
        phase = recording ? .recording : .idle
        self.meetingId = meetingId
    }
    var isRecording: Bool { phase == .recording }
    func reset() { phase = .idle; meetingId = nil }
    func flushDrafts() {}
    func start() {}
    func stop() {}
    nonisolated static func diarize(engine: PennelEngine, meetingId: String) throws -> UInt32 { throw PennelError.ModelNotDownloaded }
}

@MainActor
enum MeetingShare {
    static func copyFormatted(_ export: MeetingExport) {}
    static func copyMarkdown(_ export: MeetingExport) {}
    static func mail(_ export: MeetingExport) {}
    static func share(_ export: MeetingExport) {}
}

@MainActor
final class AudioScrubber {
    static let shared = AudioScrubber()
    func play(_ url: URL, fromMs ms: UInt64) {}
    func stop() {}
}

/// Shadows the imported provider helpers in this standalone QA module.
func providerModels(baseUrl: String, apiKey: String?) throws -> [RemoteModel] { [] }
func checkCliAgent(kind: ProviderKind, path: String, model: String) throws -> String { "QA: niet uitgevoerd" }
#endif
