import AVFAudio
import PennelMobileKit
import SwiftUI
import UIKit

/// First launch, spec §4.7: microphone, speech choice, keyboard with Full Access, optionally the Action Button.
struct OnboardingView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.openURL) private var openURL
    @Environment(\.scenePhase) private var scenePhase
    let close: (OnboardingStatus) -> Void

    @State private var flow = OnboardingFlow(speech: .apple)
    @State private var step = OnboardingPage.welcome
    @State private var microphone = AVAudioApplication.shared.recordPermission
    @State private var keyboard = KeyboardSetup.state(in: KeyboardLayout.groupDefaults)

    var body: some View {
        NavigationStack {
            content
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                .background(Brand.surface.color)
                .safeAreaInset(edge: .bottom) { footer }
                .toolbar {
                    ToolbarItem(placement: .topBarTrailing) {
                        Button(step == .actionButton ? Copy.Onboarding.skip : Copy.Onboarding.later) {
                            close(step == .actionButton ? .completed : .skipped)
                        }
                        .font(Theme.font(.body))
                    }
                }
                .navigationDestination(for: ProviderRoute.self) { route in
                    ProviderDetailView(providerId: route.id)
                }
        }
        .onAppear {
            flow.speech = OnboardingFlow.initialChoice(settings: model.settings, cloudSignedIn: model.engine.cloudSignedIn())
        }
        .onChange(of: scenePhase) { _, phase in
            guard phase == .active else { return }
            microphone = AVAudioApplication.shared.recordPermission
            keyboard = KeyboardSetup.state(in: KeyboardLayout.groupDefaults)
        }
    }

    @ViewBuilder private var content: some View {
        switch step {
        case .welcome: welcome
        case .microphone:
            page(Copy.Onboarding.allowMicrophone, nil) { microphoneRow }
        case .speech:
            page(Copy.Onboarding.speechTitle, Copy.Onboarding.speechDescription) { speechChoices }
        case .cloud:
            embedded(Copy.Onboarding.pennelCloud, nil) { AccountView() }
        case .provider:
            embedded(Copy.Onboarding.ownProvider, Copy.Onboarding.ownProviderNote) { ProvidersView() }
        case .keyboard:
            page(Copy.Onboarding.addKeyboard, Copy.Onboarding.fullAccessExplanation) { keyboardSetup }
        case .actionButton:
            page(Copy.Onboarding.actionButton, Copy.Onboarding.optional) { actionButton }
        }
    }

    private var welcome: some View {
        VStack(spacing: Brand.Spacing.s5) {
            Spacer()
            WaveMark(level: 0.5)
                .frame(width: 64, height: 48)
                .padding(Brand.Spacing.s4)
                .background(Brand.pillBackground.color, in: RoundedRectangle(cornerRadius: Brand.Radius.panel))
            Text(Copy.Onboarding.welcome)
                .font(.instrumentSerif(Brand.FontSize.displayTitle))
            Text(Copy.Onboarding.headline)
                .font(Theme.font(.title3))
            Text(Copy.Onboarding.welcomeNote)
                .font(Theme.font(.subheadline))
                .foregroundStyle(Brand.muted.color)
            Spacer()
        }
        .multilineTextAlignment(.center)
        .frame(maxWidth: .infinity)
        .padding(Brand.Spacing.s6)
    }

    private func page(_ title: String, _ description: String?, @ViewBuilder body: () -> some View) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Brand.Spacing.s5) {
                heading(title, description)
                body()
            }
            .padding(Brand.Spacing.s6)
        }
    }

    private func embedded(_ title: String, _ description: String?, @ViewBuilder body: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            heading(title, description)
                .padding([.horizontal, .top], Brand.Spacing.s6)
            body()
        }
    }

    private func heading(_ title: String, _ description: String?) -> some View {
        VStack(alignment: .leading, spacing: Brand.Spacing.s2) {
            Text(title)
                .font(.instrumentSerif(Brand.FontSize.displayTitle))
            if let description {
                Text(description)
                    .font(Theme.font(.body))
                    .foregroundStyle(Brand.muted.color)
            }
        }
    }

    private var microphoneRow: some View {
        HStack(alignment: .top, spacing: Brand.Spacing.s3) {
            Image(systemName: "mic")
                .foregroundStyle(Brand.accent.color)
            VStack(alignment: .leading, spacing: Brand.Spacing.s2) {
                Text(Copy.Onboarding.microphone).font(Theme.font(.headline, weight: .semibold))
                Text(Copy.Onboarding.microphoneDescription)
                    .font(Theme.font(.subheadline))
                    .foregroundStyle(Brand.muted.color)
                switch microphone {
                case .granted:
                    Label(Copy.Onboarding.granted, systemImage: "checkmark")
                        .font(Theme.font(.subheadline, weight: .medium))
                case .denied:
                    Text(Copy.Onboarding.microphoneDenied)
                        .font(Theme.font(.subheadline))
                    Button(Copy.Settings.openSettings, action: openSystemSettings)
                        .buttonStyle(.bordered)
                default:
                    Button(Copy.Onboarding.grantAccess) {
                        Task {
                            _ = await MicCapture.requestPermission()
                            microphone = AVAudioApplication.shared.recordPermission
                        }
                    }
                    .buttonStyle(.bordered)
                }
            }
        }
        .padding(Brand.Spacing.s4)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Brand.subtle.color, in: RoundedRectangle(cornerRadius: Brand.Radius.panel))
    }

    private var speechChoices: some View {
        VStack(spacing: Brand.Spacing.s2) {
            ForEach(SpeechChoice.allCases, id: \.self) { choice in
                Button { flow.speech = choice } label: {
                    HStack {
                        VStack(alignment: .leading, spacing: Brand.Spacing.s1) {
                            Text(choice.title).font(Theme.font(.headline, weight: .semibold))
                            Text(choice.detail)
                                .font(Theme.font(.subheadline))
                                .foregroundStyle(Brand.muted.color)
                        }
                        Spacer()
                        if flow.speech == choice {
                            Image(systemName: "checkmark").foregroundStyle(Brand.accent.color)
                        }
                    }
                    .padding(Brand.Spacing.s4)
                    .contentShape(Rectangle())
                    .background(
                        flow.speech == choice ? Brand.selection.color : Brand.subtle.color,
                        in: RoundedRectangle(cornerRadius: Brand.Radius.panel))
                }
                .buttonStyle(.plain)
                .accessibilityAddTraits(flow.speech == choice ? .isSelected : [])
            }
        }
    }

    private var keyboardSetup: some View {
        VStack(alignment: .leading, spacing: Brand.Spacing.s4) {
            Text(Copy.Onboarding.keyboardSteps).font(Theme.font(.body))
            Label(
                keyboard == .fullAccess ? Copy.Onboarding.keyboardReady : Copy.Onboarding.keyboardNotSeen,
                systemImage: keyboard == .fullAccess ? "checkmark" : "keyboard")
                .font(Theme.font(.subheadline, weight: .medium))
                .foregroundStyle(keyboard == .fullAccess ? Brand.foreground.color : Brand.muted.color)
            Button(Copy.Settings.openSettings, action: openSystemSettings)
                .buttonStyle(.bordered)
        }
    }

    private var actionButton: some View {
        VStack(alignment: .leading, spacing: Brand.Spacing.s4) {
            Text(Copy.Settings.quickStartActionButton)
            Text(Copy.Settings.quickStartBackTap)
            Text(Copy.Settings.quickStartControlCenter)
        }
        .font(Theme.font(.body))
    }

    private var footer: some View {
        HStack(spacing: Brand.Spacing.s3) {
            if let previous = flow.step(before: step) {
                Button(Copy.Onboarding.back) { step = previous }
                    .font(Theme.font(.body))
            }
            if step != .welcome, let index = flow.steps.firstIndex(of: step) {
                Text(Copy.Onboarding.stepCount(index, of: flow.steps.count - 1))
                    .font(Theme.font(.footnote))
                    .foregroundStyle(Brand.muted.color)
            }
            Spacer()
            Button(action: advance) {
                Text(primaryLabel)
                    .font(Theme.font(.headline, weight: .semibold))
                    .foregroundStyle(Brand.buttonText.color)
                    .padding(.horizontal, Brand.Spacing.s3)
            }
            .buttonStyle(.borderedProminent)
            .tint(Brand.button.color)
            .controlSize(.large)
        }
        .padding(Brand.Spacing.s4)
        .background(Brand.surface.color)
    }

    private var primaryLabel: String {
        switch step {
        case .welcome: Copy.Onboarding.start
        case .actionButton: Copy.Onboarding.done
        default: Copy.Onboarding.next
        }
    }

    private func advance() {
        if [.speech, .cloud, .provider].contains(step) { applySpeech() }
        guard let next = flow.step(after: step) else {
            close(.completed)
            return
        }
        step = next
    }

    private func applySpeech() {
        let choice = flow.speech
        let signedIn = model.engine.cloudSignedIn()
        model.update { OnboardingFlow.apply(choice, to: &$0, cloudSignedIn: signedIn, hasKey: Keychain.hasKey(for:)) }
    }

    private func openSystemSettings() {
        if let url = URL(string: UIApplication.openSettingsURLString) { openURL(url) }
    }
}
