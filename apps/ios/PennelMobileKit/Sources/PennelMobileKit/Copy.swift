import Foundation

/// Dutch strings from the iPhone design spec, copied exactly. Shared labels come from `apps/desktop/src/views/*.ts`.
public enum Copy {
    public static func minutesAndSeconds(_ seconds: TimeInterval) -> String {
        let total = max(0, Int(seconds.rounded(.down)))
        return "\(total / 60):" + String(format: "%02d", total % 60)
    }

    public enum Tab {
        public static let meetings = "Meetings"
        public static let dictate = "Dicteren"
        public static let ask = "Vraag"
        public static let settings = "Instellingen"
    }

    // MARK: 4.2 Dicteren
    public enum Dictation {
        public static let sessionCard = "Dicteersessie"
        public static let sessionOff = "Uit"
        public static let startSession = "Start sessie"
        public static let stopSession = "Stop sessie"
        public static func sessionReady(remaining: TimeInterval) -> String {
            "Klaar · nog \(minutesAndSeconds(remaining))"
        }
        public static let sessionReadyExplanation = "De microfoon staat klaar voor het toetsenbord. Pennel luistert pas als je op de dicteerknop tikt."
        public static let listening = "Luistert…"
        public static let processing = "Pennel verwerkt je dictaat."
        public static let dictateToClipboard = "Dicteer naar klembord"
        public static let history = "Geschiedenis"
        public static let historyDesktopTitle = "Dictaten"
        public static let historyEmpty = "Nog geen dictaten."
        public static let copy = "Kopiëren"
        public static let retry = "Opnieuw proberen"
    }

    // MARK: 4.4 Instellingen
    public enum Settings {
        public static let providers = "Providers"
        public static let models = "Modellen"
        public static let dictation = "Dicteren"
        public static let meetings = "Meetings"
        public static let recording = "Opnemen"
        public static let vocabulary = "Woordenlijst"
        public static let styles = "Stijlen"
        public static let account = "Account"

        public static let cliAgentsDisabled = "Werkt alleen op een computer."
        public static let localModelsDisabled = "Lokale modellen komen later naar iPhone. Apple-dicteren werkt al zonder internet."
        public static let echoCancellationDisabled = "Niet nodig op iPhone: Pennel neemt alleen de microfoon op."
        public static let recordingRulesDisabled = "Op iPhone neemt Pennel alleen op wanneer jij op Nieuwe opname tikt."
        public static let perAppStyleDisabled = "iOS laat Pennel niet zien in welke app je typt. Op iPhone geldt de standaardstijl."

        public static let modeFast = "Snel"
        public static let modeCleaned = "Opgekuist"
        public static let keyboard = "Toetsenbord"
        public static let quickStart = "Snel starten"
        public static let openSettings = "Open Instellingen"
        public static let sessionDuration5 = "5 min"
        public static let sessionDuration15 = "15 min"
        public static let sessionDuration60 = "1 uur"
        public static let defaultDeviceName = "iPhone"
    }

    // MARK: 4.5 Toetsenbord
    public enum Keyboard {
        public static let abc = "ABC"
        public static let fullAccessNeeded = "Zet Volledige toegang aan om te dicteren. Typen werkt ook zonder."
        public static let openSettings = "Open Instellingen"
        public static let startPennel = "Start Pennel"
        public static let startPennelHint = "Pennel opent even en zet de microfoon klaar."
        public static let ready = "Tik of hou ingedrukt om te praten"
        public static let listening = "Luistert…"
        public static let processing = "Pennel verwerkt je dictaat."
        public static let retry = "Opnieuw proberen"
        public static let insertLast = "Laatste dictaat invoegen"
        public static let sessionOff = "Sessie uit"
        public static let openPennelFallback = "Open Pennel en tik op Start sessie."
        public static let micReadyLiveActivity = "Pennel staat klaar"
        public static let micReadyInApp = "Microfoon staat klaar. Ga terug met ◀ linksboven."
    }

    // MARK: 4.6 Snel starten zonder toetsenbord
    public enum Intents {
        public static let dictateToClipboard = "Dicteer naar klembord"
        public static let startDictationSession = "Start dicteersessie"
        public static let stopDictationSession = "Stop dicteersessie"
        public static let startRecording = "Start opname"
        public static let stopRecording = "Stop opname"
    }

    // MARK: 4.7 Eerste start
    public enum Onboarding {
        public static let headline = "Drie dingen en je kan dicteren in elke app."
        public static let allowMicrophone = "Microfoon toestaan"
        public static let chooseSpeech = "Spraak kiezen"
        public static let pennelCloud = "Pennel Cloud"
        public static let appleDictation = "Apple-dicteren"
        public static let ownProvider = "Eigen provider"
        public static let addKeyboard = "Toetsenbord toevoegen en Volledige toegang aanzetten"
        public static let fullAccessExplanation = "Volledige toegang laat het toetsenbord met de Pennel-app praten. Het toetsenbord gebruikt geen internet en bewaart niet wat je typt."
        public static let actionButton = "Action Button instellen"
    }

    // MARK: 5.3 Dicteersessie in de app
    public enum Session {
        public static func liveActivityReady(remaining: TimeInterval) -> String {
            "Pennel staat klaar · \(minutesAndSeconds(remaining))"
        }
        public static let stop = "Stop"
        public static let stoppedByCall = "Sessie gestopt door een oproep."
    }
}
