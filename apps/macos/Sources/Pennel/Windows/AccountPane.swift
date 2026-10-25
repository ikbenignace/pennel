import PennelKit
import SwiftUI

struct AccountPane: View {
    var body: some View {
        PaneSection(title: "Pennel Cloud") {
            Text("Beschikbaar in de nieuwe Pennel-app.")
                .font(.figtree(12))
                .foregroundStyle(Theme.muted)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}
