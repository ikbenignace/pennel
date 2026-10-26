import Observation

enum AppTab: Hashable { case meetings, dictate, ask, settings }

/// What an intent asks the interface to show once the app is in front.
@MainActor
@Observable
final class AppRouter {
    static let shared = AppRouter()

    var selectedTab = AppTab.meetings
    var showsSessionReturn = false
    private(set) var dictationRequests = 0
    @ObservationIgnored private var handledDictationRequests = 0

    func requestDictation() {
        selectedTab = .dictate
        dictationRequests += 1
    }

    /// True once per request, so a dictation screen created after a cold launch still starts it exactly once.
    func takeDictationRequest() -> Bool {
        guard handledDictationRequests < dictationRequests else { return false }
        handledDictationRequests = dictationRequests
        return true
    }
}
