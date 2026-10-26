import SwiftUI
import WidgetKit

@main
struct PennelWidgets: WidgetBundle {
    var body: some Widget {
        SessionLiveActivity()
        RecordingLiveActivity()
        DictateControl()
        DictationSessionControl()
        RecordingControl()
    }
}
