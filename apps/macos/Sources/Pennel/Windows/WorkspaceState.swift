import Foundation
import Observation
@preconcurrency import PennelCore

enum WorkspaceDestination: String, CaseIterable, Identifiable {
    case dictation, meetings, history, settings
    var id: Self { self }
    var title: String {
        switch self {
        case .dictation: "Dicteren"
        case .meetings: "Vergaderingen"
        case .history: "Geschiedenis"
        case .settings: "Instellingen"
        }
    }
    var symbol: String {
        switch self {
        case .dictation: "mic"
        case .meetings: "note.text"
        case .history: "clock.arrow.circlepath"
        case .settings: "slider.horizontal.3"
        }
    }
}

enum MeetingDetailTab: Hashable { case notes, transcript, ask }
enum OwnedDictationKind: Sendable { case pad, practice }
enum OwnedDictationPhase: Sendable { case idle, listening, working, done, error }

@MainActor
@Observable
final class OwnedDictationSession {
    let id: String
    let kind: OwnedDictationKind
    var phase: OwnedDictationPhase = .idle
    var text = ""
    var error: String?
    var targetName: String?
    var level: Float = 0
    var busy: Bool { phase == .listening || phase == .working }

    init(id: String = UUID().uuidString, kind: OwnedDictationKind) {
        self.id = id
        self.kind = kind
    }
}

@MainActor
protocol OwnedDictationManaging: AnyObject {
    @discardableResult func openSession(kind: OwnedDictationKind) -> OwnedDictationSession?
    func toggleSession(_ id: String)
    func beginSession(_ id: String)
    func submitSession(_ id: String)
    func cancelSession(_ id: String)
    func closeSession(_ id: String)
    func cancelExternalPadDelivery()
    func shutdownOwnedSession()
}

@MainActor
@Observable
final class WorkspaceState {
    var destination: WorkspaceDestination = .dictation
    var dictationPadOpen = false
    var settingsTab: SettingsTab = .providers
    var providerSelection: String?
    var meetingQuery = ""
    var meetingSelection: String?
    var meetingTabs: [String: MeetingDetailTab] = [:]
    var meetingNoteDrafts: [String: String] = [:]
    var templateEditing: TemplatesSection.EditorTarget?
    var historyRetrying: Set<String> = []
    var historyMessage: String?
    var chatDrafts: [String: String] = [:]
    var askingMeetings = false
    var showingRecording = false
    var recentMeetings: [MeetingInfo] = []
    var padText = ""
    var padError: String?
    var session: OwnedDictationSession?
    var onboardingEngine = "local"
    var onboardingProvider = ""
    var onboardingModel = ""
    var onboardingLocalModel = ""
    var onboardingBaseURL: String?
    var onboardingLanguageDraft: String?
    var onboardingLanguageEdited = false
    var onboardingInitialized = false
    var initialSetup = false
    var permissionRevision = 0
    @ObservationIgnored private var saveTask: Task<Void, Never>?
    @ObservationIgnored private var persistPad: ((String) throws -> Void)?

    func attachPad(text: String, save: @escaping (String) throws -> Void) {
        padText = text
        persistPad = save
    }

    func editPad(_ text: String) {
        padText = text
        saveTask?.cancel()
        saveTask = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(250))
            guard !Task.isCancelled else { return }
            self?.flushPad()
        }
    }

    func retainUnsavedPad(_ text: String) {
        saveTask?.cancel()
        saveTask = nil
        padText = text
        padError = "Dicteerpad bewaren mislukt. Je tekst staat hier; probeer opnieuw."
    }

    @discardableResult
    func flushPad() -> Bool {
        saveTask?.cancel()
        saveTask = nil
        do { try persistPad?(padText); padError = nil; return true }
        catch { padError = "Dicteerpad bewaren mislukt. Probeer opnieuw."; return false }
    }

    func selectMeeting(_ id: String) {
        destination = .meetings
        showingRecording = false
        askingMeetings = false
        meetingSelection = id
    }

    func recordingFinished(_ id: String) {
        let visible = destination == .meetings && showingRecording
        showingRecording = false
        if visible { selectMeeting(id) }
    }

    func flushMeetingNotes(save: (String, String) throws -> Void) -> Bool {
        var succeeded = true
        for (id, text) in meetingNoteDrafts {
            do { try save(id, text); if meetingNoteDrafts[id] == text { meetingNoteDrafts.removeValue(forKey: id) } }
            catch { succeeded = false }
        }
        return succeeded
    }
}
