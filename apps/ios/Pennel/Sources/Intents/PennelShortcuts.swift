import AppIntents

struct PennelShortcuts: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: DictateToClipboardIntent(),
            phrases: [
                "Dicteer met \(.applicationName)",
                "Dicteer naar klembord met \(.applicationName)",
            ],
            shortTitle: "Dicteer naar klembord",
            systemImageName: "mic"
        )
        AppShortcut(
            intent: StartDictationSessionIntent(),
            phrases: [
                "Start een dicteersessie met \(.applicationName)",
                "Start dicteersessie in \(.applicationName)",
            ],
            shortTitle: "Start dicteersessie",
            systemImageName: "mic.circle"
        )
        AppShortcut(
            intent: StopDictationSessionIntent(),
            phrases: ["Stop de dicteersessie van \(.applicationName)"],
            shortTitle: "Stop dicteersessie",
            systemImageName: "mic.slash"
        )
        AppShortcut(
            intent: StartMeetingIntent(),
            phrases: [
                "Start een opname met \(.applicationName)",
                "Start opname in \(.applicationName)",
            ],
            shortTitle: "Start opname",
            systemImageName: "waveform"
        )
        AppShortcut(
            intent: StopMeetingIntent(),
            phrases: ["Stop de opname van \(.applicationName)"],
            shortTitle: "Stop opname",
            systemImageName: "stop.circle"
        )
    }
}
