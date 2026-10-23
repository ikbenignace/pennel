import AVFoundation
import Combine
@preconcurrency import MusliCore
import MusliKit
import SwiftUI

struct OnboardingView: View {
    @Bindable var state: AppState
    @State private var micOK = Permissions.microphone == .authorized
    @State private var axOK = Permissions.accessibilityTrusted
    @State private var keyOK = false
    private let timer = Timer.publish(every: 1, on: .main, in: .common).autoconnect()

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Welkom bij Musli")
                    .font(.custom("Instrument Serif", size: 30))
                Text("Drie dingen en je kan dicteren in elke app.")
                    .foregroundStyle(Theme.muted)
            }
            step(
                ok: micOK,
                title: "Microfoon",
                detail: "Zodat Musli kan horen wat je dicteert.",
                actionTitle: micOK ? nil : "Geef toegang",
                action: requestMicrophone
            )
            step(
                ok: axOK,
                title: "Toegankelijkheid",
                detail: "Nodig voor de sneltoets en om tekst in andere apps te plakken.",
                actionTitle: axOK ? nil : "Open instellingen",
                action: Permissions.promptAccessibility
            )
            step(
                ok: keyOK,
                title: "Provider",
                detail: "Plak een API-sleutel bij Instellingen → Providers, of download daar een lokaal model.",
                actionTitle: nil,
                action: {}
            )
            Text("Tip: zet in Systeeminstellingen → Toetsenbord de 🌐/fn-toets op ‘Doe niets’, anders opent macOS de emoji-kiezer.")
                .font(.figtree(12))
                .foregroundStyle(Theme.muted)
            Spacer()
        }
        .padding(32)
        .frame(minWidth: 480, minHeight: 500, alignment: .topLeading)
        .background(Theme.paper)
        .onReceive(timer) { _ in refresh() }
        .onAppear(perform: refresh)
    }

    private func refresh() {
        micOK = Permissions.microphone == .authorized
        axOK = Permissions.accessibilityTrusted
        state.downloads.refresh()
        keyOK = state.isReady(state.settings.dictationStt)
    }

    private func requestMicrophone() {
        Task { micOK = await Permissions.requestMicrophone() }
    }

    private func step(
        ok: Bool,
        title: String,
        detail: String,
        actionTitle: String?,
        action: @escaping @MainActor () -> Void
    ) -> some View {
        HStack(alignment: .top, spacing: 12) {
            StatusDot(ok: ok).padding(.top, 6)
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.figtree(14, weight: .medium))
                Text(detail).font(.figtree(12)).foregroundStyle(Theme.muted)
            }
            Spacer()
            if let actionTitle {
                Button(actionTitle, action: action)
            } else if ok {
                Text("In orde").font(.figtree(12)).foregroundStyle(Theme.muted)
            }
        }
        .padding(14)
        .background(RoundedRectangle(cornerRadius: 12).fill(Theme.background))
    }
}
