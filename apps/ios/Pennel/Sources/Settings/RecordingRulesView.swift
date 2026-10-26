import PennelMobileKit
import SwiftUI

/// iOS lets Pennel see neither other apps' microphone use nor the calendar while recording, so the automatic rules
/// stay visible but switched off.
struct RecordingRulesView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        Form {
            Section {
                Toggle("Automatisch herkennen", isOn: .constant(false))
                Toggle("Agenda gebruiken", isOn: .constant(false))
                Text("Per app").foregroundStyle(Brand.disabledText.color)
                Text("Per meeting").foregroundStyle(Brand.disabledText.color)
            } header: {
                Text("Calls herkennen")
            } footer: {
                Text(Copy.Settings.recordingRulesDisabled)
            }
            .pennelRow()
            .disabled(true)

            Section {
                Toggle("Toestemmingszin kopiëren bij de start van een opname", isOn: Binding(
                    get: { model.settings.consentEnabled },
                    set: { value in model.update { $0.consentEnabled = value } }
                ))
            }
            .pennelRow()
        }
        .pennelList()
    }
}
