#if DEBUG && PENNEL_REFERENCE_SNAPSHOT
import AppKit
import AVFoundation
import CoreText
import Observation
@preconcurrency import PennelCore
import PennelKit
import SwiftUI

/// Compiled only by capture-native-reference.sh, which supplies an isolated application bundle.
@main
@MainActor
final class WindowSnapshot: NSObject, NSApplicationDelegate {
    private static let meetingId = "2026-10-24 0900 Projectoverleg"
    private static let title = "Projectoverleg"
    private static let start = "2026-10-24T09:00:00+02:00"
    private static let notes = "- Planning afronden\n- Volgende afspraak bevestigen"
    private static let summary = "## Besluiten\n- De planning wordt maandag afgerond.\n\n## Actiepunten\n- [ ] Volgende afspraak bevestigen."
    private var state: AppState?
    private var emptyState: AppState?

    static func main() {
        let app = NSApplication.shared
        let delegate = WindowSnapshot()
        app.delegate = delegate
        app.setActivationPolicy(.prohibited)
        app.run()
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        Task { @MainActor in
            do {
                try await run()
                NSApp.terminate(nil)
            } catch {
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
        let state = try AppState(snapshotDataDir: fixture.appendingPathComponent("with-meeting"))
        let empty = try AppState(snapshotDataDir: fixture.appendingPathComponent("empty"))
        self.state = state
        emptyState = empty
        try seedMeeting(state)
        let idle = MeetingController(state: state)
        let live = MeetingController(state: state, recording: true, meetingId: Self.meetingId)
        let calendar = CalendarProvider()
        var images: [[String: Any]] = []
        images.append(try await capture("library", view: LibraryView(state: state, meeting: idle, openRecording: {}), size: NSSize(width: 980, height: 640), output: output))
        images.append(try await capture("library-empty", view: LibraryView(state: empty, meeting: MeetingController(state: empty), openRecording: {}), size: NSSize(width: 980, height: 640), output: output))
        images.append(try await capture("settings-providers", view: SettingsView(state: state, calendar: calendar), size: NSSize(width: 900, height: 620), output: output))
        images.append(try await capture("settings-models", view: SnapshotModelsView(state: state), size: NSSize(width: 900, height: 620), output: output))
        images.append(try await capture("history-empty", view: HistoryView(state: state), size: NSSize(width: 560, height: 520), output: output))
        images.append(try await capture("live", view: MeetingWindowView(state: state, meeting: live), size: NSSize(width: 680, height: 600), output: output))
        images.append(try await capture("onboarding", view: OnboardingView(state: state), size: NSSize(width: 520, height: 560), output: output))
        let apple = try AppState(snapshotDataDir: fixture.appendingPathComponent("apple-models"))
        let speech = newAccount(kind: .appleSpeech, existing: apple.settings.providers)
        let intelligence = newAccount(kind: .appleIntelligence, existing: apple.settings.providers)
        apple.settings.providers = [intelligence, speech] + apple.settings.providers
        apple.settings.dictationStt = ModelChoice(providerId: speech.id, model: "dictation")
        apple.settings.meetingStt = ModelChoice(providerId: speech.id, model: "dictation")
        apple.settings.llm = ModelChoice(providerId: intelligence.id, model: "system")
        images.append(try await capture("settings-apple-intelligence", view: SettingsView(state: apple, calendar: calendar), size: NSSize(width: 900, height: 620), output: output))
        images.append(try await capture("settings-apple-models", view: SnapshotModelsView(state: apple), size: NSSize(width: 900, height: 620), output: output))
        let manifest: [String: Any] = [
            "capture_method": "NSHostingView.cacheDisplay on owned offscreen windows; no display capture",
            "platform_services": "inert QA replacements; no Keychain, permissions, calendar, audio, CLI, or provider calls",
            "presentation_sources": "Original Pennel SwiftUI views; Models pane uses the original components in the same settings container",
            "images": images,
            "fixture": [
                "meeting_id": Self.meetingId, "title": Self.title, "start_iso": Self.start,
                "duration_min": 25, "notes": Self.notes, "summary": Self.summary,
                "transcript": [
                    ["start_ms": 0, "end_ms": 8000, "speaker": "others", "text": "We ronden de planning maandag af."],
                    ["start_ms": 8000, "end_ms": 14000, "speaker": "me", "text": "Ik bevestig de volgende afspraak."]
                ],
                "live_elapsed_secs": 754,
                "providers": ["Groq", "Lokaal"], "keys": [:] as [String: Bool], "history": [] as [String]
            ] as [String: Any]
        ]
        try JSONSerialization.data(withJSONObject: manifest, options: [.prettyPrinted, .sortedKeys])
            .write(to: output.appendingPathComponent("fixture.json"), options: .atomic)
    }

    private func seedMeeting(_ state: AppState) throws {
        let folder = URL(fileURLWithPath: state.settings.meetingsDir, isDirectory: true)
            .appendingPathComponent(Self.meetingId, isDirectory: true)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        let markdown = "---\ntitle: \(Self.title)\nstart: \(Self.start)\nduration_min: 25\ntemplate: algemeen\n---\n\n\(Self.summary)\n"
        let transcript = "[00:00] **Anderen:** We ronden de planning maandag af.\n\n[00:08] **Ik:** Ik bevestig de volgende afspraak.\n"
        try markdown.write(to: folder.appendingPathComponent("meeting.md"), atomically: true, encoding: .utf8)
        try Self.notes.write(to: folder.appendingPathComponent("notes.md"), atomically: true, encoding: .utf8)
        try transcript.write(to: folder.appendingPathComponent("transcript.md"), atomically: true, encoding: .utf8)
    }

    private func registerFonts() {
        guard let resources = Bundle.main.resourceURL,
              let fonts = try? FileManager.default.contentsOfDirectory(at: resources.appendingPathComponent("Fonts"), includingPropertiesForKeys: nil) else { return }
        for font in fonts where font.pathExtension == "ttf" {
            CTFontManagerRegisterFontsForURL(font as CFURL, .process, nil)
        }
    }

    private func capture(_ name: String, view: some View, size: NSSize, output: URL) async throws -> [String: Any] {
        let window = SnapshotWindow(contentRect: NSRect(origin: NSPoint(x: -10000, y: -10000), size: size), styleMask: .borderless, backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.appearance = NSAppearance(named: .aqua)
        window.backgroundColor = NSColor(Theme.paper)
        let host = NSHostingView(rootView: view.font(.figtree(13)).foregroundStyle(Theme.ink).environment(\.colorScheme, .light))
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
        return ["file": name + ".png", "logical_width": size.width, "logical_height": size.height, "pixel_width": bitmap.pixelsWide, "pixel_height": bitmap.pixelsHigh]
    }
}

private enum SnapshotError: Error { case arguments, fixture, bitmap }

@MainActor
private final class SnapshotWindow: NSWindow {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
    override func constrainFrameRect(_ frameRect: NSRect, to screen: NSScreen?) -> NSRect { frameRect }
}

/// ModelsPane is the unchanged production view, inside its production settings components.
private struct SnapshotModelsView: View {
    let state: AppState
    @State private var tab = SettingsTab.models
    var body: some View {
        VStack(spacing: 0) {
            SegmentTabs(tabs: [(.providers, "Providers"), (.models, "Modellen"), (.dictation, "Dicteren"), (.meetings, "Meetings"), (.recording, "Opnemen"), (.dictionary, "Woordenlijst"), (.styles, "Stijlen")], selection: $tab)
                .padding(.top, 14).padding(.bottom, 10)
            ScrollView { ModelsPane(state: state).padding(.horizontal, 24).padding(.bottom, 24) }
        }
        .frame(minWidth: 760, minHeight: 520)
        .background(Theme.paper)
    }
}

enum Keychain {
    static func get(_ account: String) -> String? { nil }
    static func set(_ value: String?, for account: String) {}
}

extension HotkeyTrigger {
    var label: String {
        switch self {
        case .fn, .controlSuper: "fn"
        case .rightOption: "rechter ⌥"
        case .rightCommand: "rechter ⌘"
        case .rightControl: "rechter ⌃"
        }
    }
}

enum Permissions {
    static var microphone: AVAuthorizationStatus { .notDetermined }
    static var accessibilityTrusted: Bool { false }
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
