import AppKit
import AVFoundation
import Combine
@preconcurrency import PennelCore
import PennelKit
import Speech
import SwiftUI

struct OnboardingView: View {
    @Environment(\.colorScheme) private var colorScheme
    @Bindable var state: AppState
    var onFinish: () -> Void = {}
    @State private var apiKey = ""
    @State private var message: String?
    @State private var micOK = Permissions.microphone == .authorized
    @State private var axOK = Permissions.accessibilityTrusted
    @State private var speechOK = SFSpeechRecognizer.authorizationStatus() == .authorized
    @State private var holdingKeycap = false
    private let timer = Timer.publish(every: 1, on: .main, in: .common).autoconnect()

    private var step: OnboardingStep { state.settings.onboarding.resumeStep }
    private var stage: Int {
        switch step { case .language, .welcome: 0; case .engine: 1; case .permissions: 2; case .practice, .voice: 3 }
    }
    private var session: OwnedDictationSession? { state.workspace.session?.kind == .practice ? state.workspace.session : nil }
    private var engine: String {
        get { state.workspace.onboardingEngine }
        nonmutating set { state.workspace.onboardingEngine = newValue }
    }
    private var model: LocalModel? {
        state.downloads.models.first { $0.id == state.workspace.onboardingLocalModel }
            ?? state.downloads.models.first { $0.id == "whisper-large-v3-turbo" }
            ?? state.downloads.models.first
    }
    private var customAccount: ProviderAccount? { state.account(state.workspace.onboardingProvider) }
    private var customProviders: [ProviderAccount] {
        state.settings.providers.filter { $0.kind != .local && $0.kind != .appleSpeech && $0.kind != .pennelCloud && supports(kind: $0.kind, task: .dictationStt) }
    }
    private var customKinds: [ProviderKind] {
        [.openAi, .openRouter, .groq, .mistral, .ollama, .lmStudio, .custom].filter { supports(kind: $0, task: .dictationStt) }
    }
    private var customKind: ProviderKind? {
        customAccount?.kind ?? customKinds.first { state.workspace.onboardingProvider == "new:\(String(describing: $0))" }
    }
    private var providerOptions: [(String, String)] {
        customProviders.map { ($0.id, $0.name) } + customKinds.filter { kind in !customProviders.contains { $0.kind == kind } }.map { ("new:\(String(describing: $0))", $0.label) }
    }
    private var localReady: Bool { model.map { state.downloads.states[$0.id] == .downloaded } ?? false }
    private var downloading: Bool { model.map { state.downloads.isDownloading($0.id) } ?? false }
    private var canContinue: Bool {
        switch step {
        case .language, .welcome: true
        case .engine: engine == "apple" ? ProviderKind.appleSpeech.isPlatformSupported : engine != "local" || !downloading
        case .permissions: state.dictationSetup.canRecord
        case .practice, .voice: session?.phase == .done
        }
    }
    private var nextTitle: String {
        switch step {
        case .language, .welcome: return "Aan de slag"
        case .engine:
            if engine == "custom" { return "Gebruik provider" }
            if engine != "local" || localReady { return "Verder" }
            if downloading { return "Downloaden…" }
            return model.flatMap { state.downloads.errors[$0.id] } == nil ? "Download model" : "Opnieuw proberen"
        case .permissions: return canContinue && !state.dictationSetup.canUseShortcut ? "Verder zonder \(state.settings.hotkey.label)" : "Verder"
        case .practice, .voice: return "Open Pennel"
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            ZStack(alignment: .topLeading) {
                Text("Pennel").font(.figtree(11, weight: .medium)).foregroundStyle(Theme.muted)
                    .padding(.leading, 20).padding(.top, 14)
                if step != .welcome && step != .language {
                    HStack(spacing: 5) {
                        ForEach(0..<4) { index in
                            Capsule().fill(index <= stage ? Theme.ink : Theme.line).frame(width: 29, height: 3)
                        }
                    }
                    .frame(maxWidth: .infinity).padding(.top, 30)
                    .accessibilityLabel("Stap \(stage + 1) van 4")
                }
            }
            .frame(maxWidth: .infinity, alignment: .topLeading)
            .frame(height: (step == .welcome || step == .language) ? 25 : 34)
            .background(OnboardingDragRegion())
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    switch step {
                    case .language, .welcome: welcome
                    case .engine: engineChoice
                    case .permissions: permissions
                    case .practice, .voice: practice
                    }
                    if let error = message ?? state.lastError {
                        Text(error).font(.figtree(12)).foregroundStyle(Theme.accent).padding(.top, 17)
                    }
                }
                .frame(maxWidth: step == .engine ? 556 : 510, alignment: .leading)
                .padding(.horizontal, 27).padding(.top, (step == .welcome || step == .language) ? 46 : step == .engine ? 48 : 35).padding(.bottom, 24)
                .frame(maxWidth: .infinity)
            }
            footer
        }
        .frame(minWidth: 320, idealWidth: 850, maxWidth: .infinity, minHeight: 0, idealHeight: 690, maxHeight: .infinity)
        .background(Theme.paper).foregroundStyle(Theme.ink).font(.figtree(13))
        .onAppear { initialize(); refresh(); ensurePractice() }
        .onReceive(timer) { _ in refresh(); ensurePractice() }
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)) { _ in refresh(); ensurePractice() }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didBecomeKeyNotification)) { note in
            if (note.object as? NSWindow)?.identifier?.rawValue == "onboarding" { initialize(); refresh(); ensurePractice() }
        }
        .onChange(of: state.settings.onboarding.resumeStep) { old, _ in
            message = nil
            holdingKeycap = false
            if old == .practice, let session { state.dictation?.closeSession(session.id) }
            ensurePractice()
        }
        .onChange(of: session?.id) { _, id in if id == nil { holdingKeycap = false } }
        .onDisappear { if let session { state.dictation?.closeSession(session.id) }; apiKey = ""; holdingKeycap = false }
    }

    private var welcome: some View {
        VStack(spacing: 0) {
            if let image = Bundle.main.image(forResource: "Pennel") {
                Image(nsImage: image).resizable().scaledToFit().frame(width: 77, height: 77).padding(.bottom, 18)
            } else {
                HStack(alignment: .lastTextBaseline, spacing: 3) {
                    Text("pennel").font(.custom("Instrument Serif", size: 43))
                    Circle().fill(Theme.accent).frame(width: 5, height: 5).padding(.bottom, 7)
                }.padding(.bottom, 23)
            }
            Text("Welkom bij Pennel").font(.figtree(28, weight: .semibold))
            Text("Spreek. Pennel schrijft waar je cursor staat.").font(.figtree(14)).foregroundStyle(Theme.muted).padding(.top, 10)
            HStack(spacing: 28) {
                keycap
                VStack(alignment: .leading, spacing: 9) {
                    Text("Je volgende bericht").font(.figtree(11)).foregroundStyle(Theme.muted)
                    Text("Zullen we morgen om tien uur afspreken?").font(.figtree(16))
                }
                .padding(.vertical, 16).frame(maxWidth: .infinity, alignment: .leading)
                .overlay(alignment: .top) { Rectangle().fill(Theme.line).frame(height: 1) }
                .overlay(alignment: .bottom) { Rectangle().fill(Theme.line).frame(height: 1) }
            }.padding(.top, 39)
            Text("Hou \(state.settings.hotkey.label) in. Praat. Laat los.").font(.figtree(12)).foregroundStyle(Theme.muted).padding(.top, 24)
            Text("Dicteren en meetingnotities. Zonder verplicht account.").font(.figtree(12)).foregroundStyle(Theme.muted).padding(.top, 30)
        }
        .frame(maxWidth: .infinity)
    }

    private func title(_ text: String, subtitle: String) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(text).font(.figtree(26, weight: .semibold))
            Text(subtitle).font(.figtree(13)).foregroundStyle(Theme.muted)
        }
    }

    private var engineChoice: some View {
        VStack(alignment: .leading, spacing: 0) {
            title("Hoe wil je dicteren?", subtitle: "Kies je spraakherkenning. Je kunt dit later aanpassen.")
            HStack(spacing: 12) { Text("Taal").font(.figtree(12)).foregroundStyle(Theme.muted); languagePicker }
                .padding(.top, 23).padding(.bottom, 30)
            SegmentTabs(tabs: [("local", "Lokaal"), ("apple", "Apple-dicteren"), ("custom", "Eigen provider")], selection: Binding(get: { engine }, set: { value in
                if downloading && value != "local", let model { state.downloads.cancel(model.id) }
                engine = value; message = nil
                if engine == "custom" { initializeCustomModel() }
            }), disabledTabs: ProviderKind.appleSpeech.isPlatformSupported ? [] : ["apple"])
                .accessibilityElement(children: .contain)
                .help(ProviderKind.appleSpeech.isPlatformSupported ? "" : "Apple-dicteren vereist macOS 26 of nieuwer.")
            Group {
                if engine == "local" { localChoice }
                else if engine == "apple" { appleChoice }
                else { customChoice }
            }.padding(.top, 31)
        }
    }

    private var languagePicker: some View {
        PennelPopup(label: "Taal", options: [(String?.none, "Automatisch"), (.some("nl"), "Nederlands"), (.some("en"), "Engels"), (.some("fr"), "Frans"), (.some("de"), "Duits")], selection: Binding(get: { selectedLanguage }, set: { state.workspace.onboardingLanguageDraft = $0; state.workspace.onboardingLanguageEdited = true }))
            .frame(width: 132)
    }

    private var localChoice: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Je audio blijft op je apparaat.").font(.figtree(17, weight: .medium))
            Text("Pennel zet je spraak zelf om naar tekst. Daarvoor download je eenmalig een spraakmodel.")
                .font(.figtree(13)).foregroundStyle(Theme.muted).lineSpacing(5).frame(maxWidth: descriptionWidth, alignment: .leading).padding(.top, 8)
            if let model {
                HStack(spacing: 12) {
                    PennelIcon.download.image.resizable().scaledToFit().frame(width: 18, height: 18).foregroundStyle(Theme.muted)
                    VStack(alignment: .leading, spacing: 3) {
                        Text(model.name).font(.figtree(12, weight: .medium))
                        Text("\(model.sizeBytes / 1_000_000) MB · eenmalige download").font(.figtree(11)).foregroundStyle(Theme.muted)
                    }
                    Spacer()
                    if localReady { HStack(spacing: 5) { PennelIcon.check.image.resizable().scaledToFit().frame(width: 13, height: 13); Text("Klaar") }.font(.figtree(11)).foregroundStyle(Theme.muted) }
                }.padding(.top, 28)
                if let progress = state.downloads.progress[model.id] {
                    ProgressView(value: progress).tint(Theme.ink).padding(.top, 15)
                    HStack {
                        Text("\(Int(progress * 100))%")
                        Spacer()
                        Button("Annuleer") { state.downloads.cancel(model.id) }.buttonStyle(.plain)
                    }.font(.figtree(11)).foregroundStyle(Theme.muted).padding(.top, 6)
                }
                if let error = state.downloads.errors[model.id] {
                    VStack(alignment: .leading, spacing: 8) {
                        Text(error).font(.figtree(12)).foregroundStyle(Theme.accent)
                        Button("Gebruik Apple-dicteren") {
                            engine = "apple"
                            message = nil
                        }
                        .buttonStyle(.plain).font(.figtree(12)).foregroundStyle(Theme.accent)
                        .disabled(!ProviderKind.appleSpeech.isPlatformSupported)
                        .help(ProviderKind.appleSpeech.isPlatformSupported ? "" : "Apple-dicteren vereist macOS 26 of nieuwer.")
                    }.padding(.top, 14)
                }
            }
        }.frame(maxWidth: .infinity, minHeight: 220, alignment: .topLeading)
    }

    private var appleChoice: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Met de spraakherkenning van macOS.").font(.figtree(17, weight: .medium))
            Text("Laat Apple je spraak omzetten naar tekst. macOS beheert de benodigde taalmodellen.").font(.figtree(13)).foregroundStyle(Theme.muted)
                .lineSpacing(5).frame(maxWidth: descriptionWidth, alignment: .leading)
            HStack(spacing: 12) {
                PennelIcon.audioLines.image.resizable().scaledToFit().frame(width: 18, height: 18)
                VStack(alignment: .leading, spacing: 3) {
                    Text("Apple-dicteren").font(.figtree(12, weight: .medium)).foregroundStyle(Theme.ink)
                    Text("Beschikbaar vanaf macOS 26").font(.figtree(11))
                }
            }.foregroundStyle(Theme.muted).padding(.top, 20)
            if !ProviderKind.appleSpeech.isPlatformSupported {
                Text("Apple-dicteren is niet beschikbaar op deze versie van macOS. Kies Lokaal of je eigen provider.").font(.figtree(12)).foregroundStyle(Theme.accent)
            }
        }.frame(maxWidth: .infinity, minHeight: 220, alignment: .topLeading)
    }

    private var customChoice: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Gebruik de provider die je al kent.").font(.figtree(17, weight: .medium))
            Text("Je audio wordt naar je gekozen provider gestuurd.").font(.figtree(13)).foregroundStyle(Theme.muted).padding(.top, 8)
            VStack(spacing: 0) {
                providerRow("Provider") {
                    PennelPopup(label: "Eigen provider", options: providerOptions, selection: Binding(get: { state.workspace.onboardingProvider }, set: { state.workspace.onboardingProvider = $0; initializeCustomModel() }))
                        .frame(width: 132, height: 30)
                }
                if let kind = customKind {
                    if requiresKey(kind: kind) {
                        providerRow("API-sleutel") {
                            providerInput {
                                SecureField(customAccount.map { state.hasKey($0) } == true ? "Ingesteld · laat leeg om te behouden" : "Plak je API-sleutel", text: $apiKey)
                                    .accessibilityLabel("API-sleutel")
                            }
                        }
                    }
                    providerRow("Model") {
                        providerInput { TextField("Modelnaam", text: Binding(get: { state.workspace.onboardingModel }, set: { state.workspace.onboardingModel = $0 })).accessibilityLabel("Model") }
                    }
                    if [.custom, .ollama, .lmStudio].contains(kind) {
                        providerRow("Serveradres") {
                            providerInput { TextField("https://…", text: Binding(get: { state.workspace.onboardingBaseURL ?? customAccount?.baseUrl ?? defaultBaseUrl(kind: kind) }, set: { state.workspace.onboardingBaseURL = $0 })).accessibilityLabel("Serveradres") }
                        }
                    }
                }
            }.padding(.top, 22)
            Text("Je API-sleutel wordt alleen in de sleutelopslag van je apparaat bewaard.").font(.figtree(11)).foregroundStyle(Theme.muted).padding(.top, 13)
        }.frame(maxWidth: .infinity, minHeight: 220, alignment: .topLeading)
    }

    private func providerRow<Content: View>(_ label: String, @ViewBuilder content: () -> Content) -> some View {
        HStack(spacing: 20) {
            Text(label).font(.figtree(12))
            Spacer(minLength: 0)
            content()
        }
        .padding(.vertical, 13).frame(minHeight: 54)
        .overlay(alignment: .bottom) { Rectangle().fill(Theme.line).frame(height: 1) }
    }

    private func providerInput<Content: View>(@ViewBuilder content: () -> Content) -> some View {
        content().font(.figtree(12)).textFieldStyle(.plain).padding(.horizontal, 8)
            .frame(width: 180, height: 30)
            .background(RoundedRectangle(cornerRadius: 6).fill(Theme.field))
            .overlay(RoundedRectangle(cornerRadius: 6).stroke(Theme.line))
    }

    private var descriptionWidth: CGFloat {
        let font = NSFont(name: "Figtree-Regular", size: 13) ?? NSFont.systemFont(ofSize: 13)
        return 47 * ("0" as NSString).size(withAttributes: [.font: font]).width
    }

    private var permissions: some View {
        VStack(alignment: .leading, spacing: 0) {
            title("Luisteren. En tekst plaatsen.", subtitle: "Zo kan Pennel ook in je andere apps werken.").padding(.bottom, 22)
            permission("Microfoon", detail: "Zodat Pennel kan horen wat je zegt.", symbol: "mic", ready: micOK) { requestMicrophone() }
            permission("Toegankelijkheid", detail: "Voor de sneltoets en om tekst in andere apps te plaatsen.", symbol: "keyboard", ready: axOK) { Permissions.promptAccessibility() }
            if engine == "apple" { permission("Spraakherkenning", detail: "Nodig om Apple-dicteren te gebruiken.", symbol: "waveform", ready: speechOK) { requestSpeech() } }
            if !axOK {
                Text("Geef Pennel toegang tot Toegankelijkheid om \(state.settings.hotkey.label) in andere apps te gebruiken.")
                    .font(.figtree(11)).foregroundStyle(Theme.muted).padding(.top, 6)
            }
            Text("Toegang tot je agenda en systeemaudio vragen we pas wanneer je meetings wilt opnemen.").font(.figtree(11)).foregroundStyle(Theme.muted).padding(.top, 23)
        }
    }

    private func permission(_ name: String, detail: String, symbol: String, ready: Bool, action: @escaping () -> Void) -> some View {
        HStack(alignment: .top, spacing: 14) {
            (symbol == "mic" ? PennelIcon.mic : symbol == "keyboard" ? PennelIcon.keyboard : PennelIcon.audioLines).image.resizable().scaledToFit().foregroundStyle(Theme.muted).frame(width: 19, height: 19).padding(.top, 3)
            VStack(alignment: .leading, spacing: 7) {
                Text(name).font(.figtree(13, weight: .medium))
                Text(detail).font(.figtree(12)).foregroundStyle(Theme.muted)
                if !ready { Button(permissionAction(name), action: action).padding(.top, 5) }
            }
            Spacer()
            if ready { HStack(spacing: 5) { PennelIcon.check.image.resizable().scaledToFit().frame(width: 14, height: 14); Text("In orde") }.font(.figtree(12)).foregroundStyle(Theme.muted) }
        }.padding(.vertical, 21).overlay(alignment: .bottom) { Rectangle().fill(Theme.line).frame(height: 1) }
    }

    private var practice: some View {
        VStack(alignment: .leading, spacing: 0) {
            title(session?.phase == .done ? "Precies. Zo werkt Pennel." : "Probeer je eerste dictaat.", subtitle: session?.phase == .done ? "Dezelfde sneltoets werkt waar je ook typt." : "Hou \(state.settings.hotkey.label) in, spreek de zin uit en laat los.")
            HStack(spacing: 23) {
                practiceKeycap
                VStack(alignment: .leading, spacing: 7) {
                    Text("“\(practicePhrase)”").font(.figtree(16))
                    Text("Je kunt natuurlijk ook iets anders zeggen.").font(.figtree(12)).foregroundStyle(Theme.muted)
                }
            }.padding(.top, 30).padding(.bottom, 26)
            TextEditor(text: Binding(get: { session?.text ?? "" }, set: { session?.text = $0 }))
                .font(.figtree(15)).scrollContentBackground(.hidden).padding(12).frame(height: 120)
                .background(RoundedRectangle(cornerRadius: 8).fill(Theme.field)).overlay(RoundedRectangle(cornerRadius: 8).stroke(Theme.line))
                .overlay(alignment: .topLeading) {
                    if session?.text.isEmpty != false {
                        Text("Je woorden verschijnen hier…").font(.figtree(15)).foregroundStyle(Theme.muted).padding(16).allowsHitTesting(false)
                    }
                }
                .disabled(session == nil || session?.busy == true).accessibilityLabel("Je eerste dictaat")
            HStack(spacing: 9) {
                if session?.phase == .working { ProgressView().controlSize(.small) }
                Text(practiceStatus).font(.figtree(12)).foregroundStyle(session?.error == nil ? Theme.muted : Theme.accent)
            }.frame(minHeight: 36, alignment: .leading).padding(.top, 10)
            HStack(alignment: .top, spacing: 18) {
                Button(session?.phase == .listening ? "Stop dicteren" : session?.phase == .working ? "Tekst verwerken…" : "Start dicteren") { togglePractice() }
                    .buttonStyle(.plain).font(.figtree(12)).foregroundStyle(Theme.muted)
                    .disabled(state.dictation == nil || session?.phase == .working || !state.dictationSetup.canRecord)
                DisclosureGroup("Werkt \(state.settings.hotkey.label) nog niet?") {
                    Text(state.settings.hotkey == .fn ? "Zet bij Systeeminstellingen → Toetsenbord de fn-toets op ‘Doe niets’. Je kunt ook een andere sneltoets kiezen bij Dicteren." : "Controleer de toegang tot Toegankelijkheid. Je kunt ook een andere sneltoets kiezen bij Dicteren.").font(.figtree(11)).foregroundStyle(Theme.muted).padding(.top, 8)
                }.font(.figtree(12)).foregroundStyle(Theme.muted)
            }.padding(.top, 13)
        }
    }

    private var practiceStatus: String {
        if let error = session?.error { return error }
        switch session?.phase {
        case .listening: return "Luistert… laat de toets los."
        case .working: return "Je woorden worden uitgeschreven…"
        case .done: return "Dit is je eerste dictaat."
        default: return "Hou de toets hieronder in of gebruik \(state.settings.hotkey.label)."
        }
    }

    private func permissionAction(_ name: String) -> String {
        if name == "Toegankelijkheid" { return "Open instellingen" }
        if name == "Microfoon", Permissions.microphone == .denied || Permissions.microphone == .restricted { return "Open instellingen" }
        if name == "Spraakherkenning", SFSpeechRecognizer.authorizationStatus() == .denied || SFSpeechRecognizer.authorizationStatus() == .restricted { return "Open instellingen" }
        return "Geef toegang"
    }

    private var keycap: some View {
        VStack(alignment: .leading, spacing: 14) {
            PennelIcon.globe.image.resizable().scaledToFit().frame(width: 14, height: 14).foregroundStyle(Theme.muted)
            Text(keycapLabel).font(.figtree(24, weight: .medium))
        }.padding(12).frame(width: 83, height: 83, alignment: .leading)
            .background(RoundedRectangle(cornerRadius: 11).fill(Theme.chrome)).overlay(RoundedRectangle(cornerRadius: 11).stroke(Theme.line))
            .shadow(color: Theme.line, radius: 0, y: holdingKeycap && step == .practice ? 1 : 3)
    }

    private var practiceKeycap: some View {
        keycap
            .offset(y: holdingKeycap ? 2 : 0)
            .contentShape(RoundedRectangle(cornerRadius: 11))
            .gesture(DragGesture(minimumDistance: 0)
                .onChanged { _ in startHeldPractice() }
                .onEnded { _ in stopHeldPractice() })
            .allowsHitTesting(state.dictation != nil && state.dictationSetup.canRecord && session?.phase != .working)
            .focusable(state.dictation != nil && state.dictationSetup.canRecord)
            .onKeyPress(.space, phases: [.down, .up], action: handlePracticeKey)
            .onKeyPress(.return, phases: [.down, .up], action: handlePracticeKey)
            .accessibilityElement(children: .ignore)
            .accessibilityLabel("Houd ingedrukt om te dicteren")
            .accessibilityAddTraits(.isButton)
            .accessibilityAction { togglePractice() }
    }

    private var practicePhrase: String {
        switch state.settings.language {
        case "en": "Shall we meet tomorrow at ten?"
        case "fr": "On se retrouve demain à dix heures ?"
        case "de": "Treffen wir uns morgen um zehn Uhr?"
        default: "Zullen we morgen om tien uur afspreken?"
        }
    }

    private func startHeldPractice() {
        guard !holdingKeycap, state.dictationSetup.canRecord, let manager = state.dictation else { return }
        guard let owned = session ?? manager.openSession(kind: .practice), !owned.busy else { return }
        holdingKeycap = true
        manager.beginSession(owned.id)
    }

    private func stopHeldPractice() {
        guard holdingKeycap else { return }
        holdingKeycap = false
        if let session { state.dictation?.submitSession(session.id) }
    }

    private func handlePracticeKey(_ press: KeyPress) -> KeyPress.Result {
        if press.phase == .down { startHeldPractice() }
        else { stopHeldPractice() }
        return .handled
    }

    private var keycapLabel: String {
        switch state.settings.hotkey {
        case .fn: "fn"
        case .rightOption: "⌥"
        case .rightCommand: "⌘"
        case .rightControl: "⌃"
        case .controlSuper: "⌃ ⌘"
        }
    }

    private var footer: some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 16) { footerLeading; Spacer(minLength: 0); nextButton(expanded: false) }
            VStack(alignment: .leading, spacing: 12) { footerLeading; nextButton(expanded: true) }
        }
        .font(.figtree(12)).padding(.horizontal, 25).padding(.vertical, 16)
    }

    private var footerLeading: some View {
        HStack(spacing: 15) {
            Button {
                state.settings.appearance = colorScheme == .dark ? .light : .dark
                state.save()
            } label: { (colorScheme == .dark ? PennelIcon.sun : PennelIcon.moon).image.resizable().scaledToFit().frame(width: 16, height: 16) }
            .buttonStyle(.plain).foregroundStyle(Theme.muted).accessibilityLabel("Weergave wisselen")
            if step != .welcome && step != .language { Button("Terug", action: back).buttonStyle(.plain).foregroundStyle(Theme.muted) }
            Button(step == .practice ? "Later proberen" : "Later instellen", action: skip).buttonStyle(.plain).foregroundStyle(Theme.muted).keyboardShortcut(.cancelAction)
            Text("\(stage + 1) van 4").font(.figtree(11)).foregroundStyle(Theme.muted)
        }.fixedSize(horizontal: true, vertical: false)
    }

    private func nextButton(expanded: Bool) -> some View {
        Button(action: advance) {
            HStack(spacing: 8) { Text(nextTitle); PennelIcon.arrowRight.image.resizable().scaledToFit().frame(width: 14, height: 14) }
                .font(.figtree(12, weight: .medium)).frame(maxWidth: expanded ? .infinity : nil)
                .padding(.horizontal, 14).padding(.vertical, 8)
                .foregroundStyle(Theme.buttonText).background(RoundedRectangle(cornerRadius: 6).fill(Theme.button))
        }
        .buttonStyle(.plain).disabled(!canContinue).opacity(canContinue ? 1 : 0.45)
        .fixedSize(horizontal: !expanded, vertical: false)
    }

    private func initialize() {
        guard !state.workspace.onboardingInitialized else { return }
        state.workspace.onboardingInitialized = true
        state.workspace.onboardingLanguageEdited = false
        state.workspace.onboardingLanguageDraft = nil
        if let account = state.account(state.settings.dictationStt.providerId) {
            engine = account.kind == .local ? "local" : account.kind == .appleSpeech ? "apple" : "custom"
            state.workspace.onboardingModel = state.settings.dictationStt.model
            if account.kind == .local { state.workspace.onboardingLocalModel = state.settings.dictationStt.model }
            if engine == "custom" { state.workspace.onboardingProvider = account.id; state.workspace.onboardingBaseURL = account.baseUrl }
        }
        if state.workspace.onboardingProvider.isEmpty { state.workspace.onboardingProvider = providerOptions.first?.0 ?? "" }
        if state.workspace.onboardingModel.isEmpty { state.workspace.onboardingModel = "whisper-large-v3-turbo" }
        if state.workspace.onboardingLocalModel.isEmpty { state.workspace.onboardingLocalModel = "whisper-large-v3-turbo" }
    }

    private func initializeCustomModel() {
        guard let kind = customKind else { return }
        state.workspace.onboardingModel = customAccount?.id == state.settings.dictationStt.providerId ? state.settings.dictationStt.model : defaultModel(kind: kind, task: .dictationStt)
        state.workspace.onboardingBaseURL = customAccount?.baseUrl ?? defaultBaseUrl(kind: kind)
        apiKey = ""
    }

    private func commitChoice() -> Bool {
        let choice: ModelChoice
        switch engine {
        case "local":
            guard let model, localReady else { return false }
            let account = state.settings.providers.first(where: { $0.kind == .local }) ?? newAccount(kind: .local, existing: state.settings.providers)
            if !state.settings.providers.contains(where: { $0.id == account.id }) { state.settings.providers.append(account) }
            choice = ModelChoice(providerId: account.id, model: model.id)
        case "apple":
            guard ProviderKind.appleSpeech.isPlatformSupported else { message = "Apple-dicteren vereist macOS 26 of nieuwer."; return false }
            let account = state.settings.providers.first { $0.kind == .appleSpeech } ?? newAccount(kind: .appleSpeech, existing: state.settings.providers)
            if !state.settings.providers.contains(where: { $0.id == account.id }) { state.settings.providers.append(account) }
            choice = ModelChoice(providerId: account.id, model: defaultModel(kind: .appleSpeech, task: .dictationStt))
        default:
            guard let kind = customKind else { message = "Kies eerst een provider."; return false }
            let existing = customAccount
            let account = existing ?? newAccount(kind: kind, existing: state.settings.providers)
            if requiresKey(kind: kind) && !(existing.map { state.hasKey($0) } ?? false) && apiKey.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { message = "Vul je API-sleutel in."; return false }
            let name = state.workspace.onboardingModel.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !name.isEmpty else { message = "Kies of vul het spraakmodel in."; return false }
            if !apiKey.isEmpty {
                guard state.setKey(apiKey, for: account) else { message = state.lastError; return false }
                apiKey = ""
            }
            if existing == nil { state.settings.providers.append(account); state.workspace.onboardingProvider = account.id }
            if let index = state.settings.providers.firstIndex(where: { $0.id == account.id }) {
                state.settings.providers[index].baseUrl = (state.workspace.onboardingBaseURL ?? account.baseUrl).trimmingCharacters(in: .whitespacesAndNewlines)
            }
            choice = ModelChoice(providerId: account.id, model: name)
        }
        state.settings.dictationStt = choice
        state.settings.language = selectedLanguage
        if state.workspace.initialSetup { state.settings.meetingStt = choice }
        state.save()
        return state.lastError == nil
    }

    private var selectedLanguage: String? {
        state.workspace.onboardingLanguageEdited ? state.workspace.onboardingLanguageDraft : state.settings.language
    }

    private func advance() {
        message = nil
        switch step {
        case .language, .welcome: state.setOnboarding(step: .engine)
        case .engine:
            if engine == "local", !localReady {
                if let model { state.downloads.start(model.id) }
                else { message = "Er is geen spraakmodel beschikbaar voor deze taal." }
                return
            }
            if commitChoice() { state.setOnboarding(step: .permissions) }
        case .permissions: state.setOnboarding(step: .practice); ensurePractice()
        case .practice, .voice:
            guard session?.phase == .done else { return }
            state.setOnboarding(step: .practice, status: .completed)
            onFinish()
        }
    }

    private func back() { state.setOnboarding(step: [.welcome, .welcome, .engine, .permissions][stage]) }
    private func skip() {
        if let session { state.dictation?.closeSession(session.id) }
        state.setOnboarding(step: step, status: .skipped)
        state.workspace.initialSetup = false
        apiKey = ""
        onFinish()
    }
    private func refresh() {
        micOK = Permissions.microphone == .authorized
        axOK = Permissions.accessibilityTrusted
        speechOK = SFSpeechRecognizer.authorizationStatus() == .authorized
    }
    private func requestMicrophone() {
        if Permissions.microphone == .denied || Permissions.microphone == .restricted {
            if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone") { NSWorkspace.shared.open(url) }
        } else { Task { micOK = await Permissions.requestMicrophone() } }
    }
    private func requestSpeech() {
        if SFSpeechRecognizer.authorizationStatus() == .denied || SFSpeechRecognizer.authorizationStatus() == .restricted {
            if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_SpeechRecognition") { NSWorkspace.shared.open(url) }
        } else { SFSpeechRecognizer.requestAuthorization { status in Task { @MainActor in speechOK = status == .authorized } } }
    }
    private func ensurePractice() {
        guard step == .practice, session == nil else { return }
        _ = state.dictation?.openSession(kind: .practice)
    }
    private func togglePractice() {
        guard let manager = state.dictation else { return }
        if let session { manager.toggleSession(session.id) }
        else if let fresh = manager.openSession(kind: .practice) { manager.toggleSession(fresh.id) }
    }
}

private struct OnboardingDragRegion: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView { DragView() }
    func updateNSView(_ nsView: NSView, context: Context) {}
    private final class DragView: NSView {
        override var mouseDownCanMoveWindow: Bool { true }
        override func mouseDown(with event: NSEvent) { window?.performDrag(with: event) }
    }
}
