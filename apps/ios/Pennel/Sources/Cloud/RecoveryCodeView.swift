import PennelMobileKit
import SwiftUI
import UIKit
import UniformTypeIdentifiers

enum RecoveryCodeClipboard {
    static let lifetime: TimeInterval = 120

    /// Never synced to other devices through Universal Clipboard and gone after two minutes.
    static func copy(_ code: String, to pasteboard: UIPasteboard = .general, now: Date = Date()) {
        pasteboard.setItems(
            [[UTType.plainText.identifier: code]],
            options: [.localOnly: true, .expirationDate: now.addingTimeInterval(lifetime)])
    }

    @MainActor
    static func print(_ code: String) {
        let formatter = UISimpleTextPrintFormatter(
            text: "Pennel Cloud herstelcode\n\n\(code)\n\nBewaar deze code op een veilige plek.")
        let controller = UIPrintInteractionController.shared
        controller.printInfo = {
            let info = UIPrintInfo.printInfo()
            info.jobName = "Pennel Cloud herstelcode"
            return info
        }()
        controller.printFormatter = formatter
        controller.present(animated: true)
    }
}

/// The recovery code, shown once. The code lives only in the model's memory.
struct RecoveryCodeView: View {
    let code: String
    let saved: () -> Void
    @State private var status = ""

    var body: some View {
        Section("Herstelcode") {
            Text("Bewaar deze herstelcode. Je hebt hem nodig om sync op een ander toestel aan te zetten. Zonder deze code en zonder ingelogd toestel kan niemand je clouddata nog lezen, ook wij niet.")
            HStack(spacing: Brand.Spacing.s3) {
                ForEach(Array(code.split(separator: "-").enumerated()), id: \.offset) { _, group in
                    Text(String(group)).font(.system(.title3, design: .monospaced, weight: .semibold))
                }
            }
            .accessibilityElement(children: .ignore)
            .accessibilityLabel("Herstelcode")
            .accessibilityValue(code)
            Button("Kopiëren") {
                RecoveryCodeClipboard.copy(code)
                status = "Gekopieerd"
            }
            Button("Ik heb hem bewaard", action: saved)
            Button("Afdrukken") { RecoveryCodeClipboard.print(code) }
            if !status.isEmpty {
                Text(status).font(Theme.font(.footnote)).foregroundStyle(Brand.muted.color).accessibilityAddTraits(.updatesFrequently)
            }
        }
        .pennelRow()
    }
}
