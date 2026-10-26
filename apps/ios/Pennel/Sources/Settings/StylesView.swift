import PennelMobileKit
import SwiftUI

struct StylesView: View {
    var body: some View {
        Form {
            Section {
                Button("App toevoegen…") {}
                    .disabled(true)
            } header: {
                Text("Stijl per app")
            } footer: {
                Text(Copy.Settings.perAppStyleDisabled)
            }
            .pennelRow()
        }
        .pennelList()
    }
}
