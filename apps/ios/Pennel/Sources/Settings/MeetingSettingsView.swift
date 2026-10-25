import PennelMobileKit
import SwiftUI

struct MeetingSettingsView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        Form {
            Section {
                Picker("Standaardsjabloon", selection: Binding(
                    get: { model.settings.defaultTemplate },
                    set: { id in model.update { $0.defaultTemplate = id } }
                )) {
                    ForEach(model.templates, id: \.id) { template in
                        Text(template.name).tag(template.id)
                    }
                }
            }
            .pennelRow()

            Section {
                Picker("Audio bewaren", selection: Binding(
                    get: { model.settings.meetingAudioRetentionDays },
                    set: { days in model.update { $0.meetingAudioRetentionDays = days } }
                )) {
                    Text("30 dagen").tag(UInt32(30))
                    Text("90 dagen").tag(UInt32(90))
                    Text("1 jaar").tag(UInt32(365))
                    Text("Altijd").tag(UInt32(0))
                }
            } footer: {
                Text("Notities en transcript blijven altijd staan.")
            }
            .pennelRow()

            Section {
                Toggle("Echo-onderdrukking (als je via speakers belt)", isOn: .constant(false))
                    .disabled(true)
            } footer: {
                Text(Copy.Settings.echoCancellationDisabled)
            }
            .pennelRow()
        }
        .pennelList()
        .onAppear { model.reloadTemplates() }
    }
}
