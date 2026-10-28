import AppIntents
import PennelMobileKit
import SwiftUI
import WidgetKit

struct DictateControl: ControlWidget {
    var body: some ControlWidgetConfiguration {
        StaticControlConfiguration(kind: "app.pennel.ios.control.dictate") {
            ControlWidgetButton(action: DictateToClipboardIntent()) {
                Label(ActivityCopy.current.text(.dictate), systemImage: "mic")
            }
        }
        .displayName(ActivityCopy.current.resource(.dictate))
        .description(ActivityCopy.current.resource(.dictateDescription))
    }
}

struct DictationSessionControl: ControlWidget {
    var body: some ControlWidgetConfiguration {
        StaticControlConfiguration(kind: "app.pennel.ios.control.session") {
            ControlWidgetButton(action: StartDictationSessionIntent()) {
                Label(ActivityCopy.current.text(.dictationSession), systemImage: "mic.circle")
            }
        }
        .displayName(ActivityCopy.current.resource(.dictationSession))
        .description(ActivityCopy.current.resource(.dictationSessionDescription))
    }
}

struct RecordingControl: ControlWidget {
    var body: some ControlWidgetConfiguration {
        StaticControlConfiguration(kind: "app.pennel.ios.control.recording") {
            ControlWidgetButton(action: StartMeetingIntent()) {
                Label(ActivityCopy.current.text(.recordingControl), systemImage: "waveform")
            }
        }
        .displayName(ActivityCopy.current.resource(.recordingControl))
        .description(ActivityCopy.current.resource(.recordingDescription))
    }
}
