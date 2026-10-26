import PennelMobileKit
import SwiftUI
import UIKit

/// The keyboard and quick-start subsections replace the desktop hotkey on iPhone.
struct DictationSettingsView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.openURL) private var openURL
    @State private var layout = KeyboardLayout.stored(in: KeyboardLayout.groupDefaults)

    var body: some View {
        Form {
            Section("Stand") {
                Picker("Stand", selection: binding(\.mode)) {
                    Text(Copy.Settings.modeCleaned).tag(DictationMode.clean)
                    Text(Copy.Settings.modeFast).tag(DictationMode.fast)
                }
                .pickerStyle(.inline)
                .labelsHidden()
            }
            .pennelRow()

            Section {
                Picker("Taal", selection: binding(\.language)) {
                    Text("Automatisch").tag(String?.none)
                    Text("Nederlands").tag(String?.some("nl"))
                    Text("Engels").tag(String?.some("en"))
                    Text("Frans").tag(String?.some("fr"))
                    Text("Duits").tag(String?.some("de"))
                }
                Picker("Geschiedenis bewaren", selection: binding(\.historyRetentionDays)) {
                    Text("7 dagen").tag(UInt32(7))
                    Text("30 dagen").tag(UInt32(30))
                    Text("90 dagen").tag(UInt32(90))
                    Text("Altijd").tag(UInt32(0))
                }
            }
            .pennelRow()

            Section(Copy.Settings.keyboard) {
                Picker("Indeling", selection: $layout) {
                    Text("QWERTY").tag(KeyboardLayout.qwerty)
                    Text("AZERTY").tag(KeyboardLayout.azerty)
                }
                .onChange(of: layout) { _, new in new.store(in: KeyboardLayout.groupDefaults) }
                Button(Copy.Settings.openSettings) {
                    if let url = URL(string: UIApplication.openSettingsURLString) { openURL(url) }
                }
            }
            .pennelRow()

            Section(Copy.Settings.quickStart) {
                LabeledContent("Sessieduur", value: Copy.Settings.sessionDuration5)
            }
            .pennelRow()
        }
        .pennelList()
    }

    private func binding<Value: Equatable>(_ keyPath: WritableKeyPath<Settings, Value>) -> Binding<Value> {
        Binding(get: { model.settings[keyPath: keyPath] }, set: { value in model.update { $0[keyPath: keyPath] = value } })
    }
}
