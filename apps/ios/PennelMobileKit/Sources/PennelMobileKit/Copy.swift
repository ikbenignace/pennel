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

    // MARK: 4.1 Meetings (labels from apps/desktop/src/views/meetings.ts and library.ts)
    public enum Meetings {
        public static let newMeeting = "Nieuwe meeting"
        public static let startRecording = "Start opname"
        public static let titlePlaceholder = "Titel, bv. Weekly sync"
        public static let stopRecording = "Stop opname"
        public static let yourNotes = "Je notities"
        public static let notesPlaceholder = "Typ steekwoorden… of laat het leeg voor een samenvatting."
        public static let liveTranscript = "Live transcript"
        public static let summarize = "Samenvatten"
        public static let regenerate = "Opnieuw genereren"
        public static let retry = "Opnieuw proberen"
        public static let retrySave = "Opnieuw bewaren"
        public static let playFromHere = "Afspelen vanaf dit punt"
        public static let stopAudio = "Stop audio"
        public static let share = "Delen"
        public static let copyFormatted = "Kopieer opgemaakt"
        public static let copyMarkdown = "Kopieer als Markdown"
        public static let copyWithTranscript = "Kopieer met transcript"
        public static let shareSheet = "Deel…"
        public static let copied = "Gekopieerd"
        public static let copiedWithTranscript = "Gekopieerd met transcript"
        public static let search = "Zoeken in meetings"
        public static let today = "Vandaag"
        public static let thisWeek = "Deze week"
        public static let earlier = "Eerder"
        public static let noMeetings = "Nog geen meetings."
        public static let noResults = "Geen meetings gevonden."
        public static let emptyHint = "Start een opname. Je notities en transcript komen hier."
        public static let finishingRow = "Afronden…"
        public static let recordingRow = "Opname loopt"
        public static func recordingClock(_ seconds: TimeInterval) -> String { "Opname loopt · \(minutesAndSeconds(seconds))" }
        public static let finishing = "Opname afronden…"
        public static func finishing(pending: UInt32) -> String {
            pending == 0 ? finishing : "\(finishing) \(pending) \(pending == 1 ? "stuk" : "stukken") te transcriberen"
        }
        public static let meetingTitle = "Titel van de meeting"
        public static let notes = "Notities"
        public static let transcript = "Transcript"
        public static let questions = "Vragen"
        public static let noSummary = "Nog geen samenvatting."
        public static let noTranscript = "Nog geen transcript."
        public static let transcriptionFailed = "Transcriptie mislukt"
        public static let noAudio = "Geen bewaarde audio beschikbaar."
        public static let template = "Sjabloon"
        public static let busy = "Bezig…"
        public static let chooseModel = "Kies een taalmodel om een samenvatting te maken."
        public static let waitForRecording = "Wacht tot de opname is afgerond."
        public static let notFound = "Meeting niet gevonden."
        public static let rename = "Hernoemen…"
        public static let renameTitle = "Meeting hernoemen"
        public static let delete = "Verwijder…"
        public static let deleteConfirm = "Verwijder"
        public static let deleteMessage = "Notities, transcript en audio worden verwijderd."
        public static let recognizeSpeakers = "Sprekers herkennen"
        public static let speakersUnavailable = "Sprekers herkennen is niet beschikbaar."
    }

    // MARK: 4.3 Vraag (labels from apps/desktop/src/views/meetings.ts)
    public enum Ask {
        public static let titleAll = "Vraag het je meetings"
        public static let titleMeeting = "Vraag het deze meeting"
        public static let placeholderAll = "Vraag iets over al je meetings"
        public static let placeholderMeeting = "Vraag iets over deze meeting"
        public static let exampleAll = "Bijvoorbeeld: ‘Wat hebben we beslist over het budget?’ of ‘Wanneer sprak ik laatst met Jan?’"
        public static let exampleMeeting = "Bijvoorbeeld: ‘Wat zijn de actiepunten voor mij?’"
        public static let ask = "Vraag"
        public static let writing = "Antwoord schrijven"
        public static let sourceGone = "Deze bron is intussen gewijzigd of niet meer beschikbaar."
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
        public static let quickStartExplanation = "Action Button: kies in Instellingen › Action Button de optie Opdracht en dan Dicteer naar klembord. Tik achterop: kies in Instellingen › Toegankelijkheid › Aanraken › Tik achterop een opdracht van Pennel; Pennel kan dat niet zelf instellen. Bedieningspaneel: houd het bedieningspaneel ingedrukt, tik op + en voeg Dicteer, Dicteersessie of Opname toe."
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
