import AppIntents
import SwiftUI
import WidgetKit

struct DictateControl: ControlWidget {
    var body: some ControlWidgetConfiguration {
        StaticControlConfiguration(kind: "app.pennel.ios.control.dictate") {
            ControlWidgetButton(action: DictateToClipboardIntent()) {
                Label("Dicteer", systemImage: "mic")
            }
        }
        .displayName("Dicteer")
        .description("Dicteer naar het klembord.")
    }
}

struct DictationSessionControl: ControlWidget {
    var body: some ControlWidgetConfiguration {
        StaticControlConfiguration(kind: "app.pennel.ios.control.session") {
            ControlWidgetButton(action: StartDictationSessionIntent()) {
                Label("Dicteersessie", systemImage: "mic.circle")
            }
        }
        .displayName("Dicteersessie")
        .description("Zet de microfoon klaar voor het Pennel-toetsenbord.")
    }
}

struct RecordingControl: ControlWidget {
    var body: some ControlWidgetConfiguration {
        StaticControlConfiguration(kind: "app.pennel.ios.control.recording") {
            ControlWidgetButton(action: StartMeetingIntent()) {
                Label("Opname", systemImage: "waveform")
            }
        }
        .displayName("Opname")
        .description("Start een nieuwe opname.")
    }
}
