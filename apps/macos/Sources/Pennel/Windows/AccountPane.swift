import PennelKit
import SwiftUI

struct AccountPane: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            Text("Pennel Cloud").font(.figtree(16, weight: .semibold))
            Text("Beschikbaar in de nieuwe Pennel-app.")
                .font(.figtree(12))
                .foregroundStyle(Theme.muted)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
