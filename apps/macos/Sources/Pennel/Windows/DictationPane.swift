import AppKit
@preconcurrency import PennelCore
import PennelKit
import SwiftUI
import UniformTypeIdentifiers

struct DictationPane: View {
    @Bindable var state: AppState

    var body: some View {
        VStack(spacing: 16) {
            PaneSection(title: "Stand") {
                Picker("", selection: $state.settings.mode) {
                    Text("Opgekuist — euh's weg, zelfcorrecties verwerkt, stijl per app").tag(DictationMode.clean)
                    Text("Snel — letterlijk wat je zei, zonder taalmodel").tag(DictationMode.fast)
                }
                .pickerStyle(.radioGroup)
                .labelsHidden()
            }
            PaneSection(title: "Sneltoets", subtitle: "Ingedrukt houden om te praten, twee keer tikken voor handenvrij. Houd Shift erbij voor een opdracht over de geselecteerde tekst.") {
                SegmentTabs(tabs: [(.fn, "fn"), (.rightOption, "Rechter Option"), (.rightCommand, "Rechter Command"), (.rightControl, "Rechter Ctrl")], selection: $state.settings.hotkey)
            }
            PaneSection(title: "Taal") {
                PennelPopup(label: "Taal", options: [(String?.none, "Automatisch"), (.some("nl"), "Nederlands"), (.some("en"), "Engels"), (.some("fr"), "Frans"), (.some("de"), "Duits")], selection: $state.settings.language)
                .frame(maxWidth: 220)
            }
            PaneSection(title: "Geschiedenis bewaren") {
                PennelPopup(label: "Geschiedenis bewaren", options: [(UInt32(7), "7 dagen"), (UInt32(30), "30 dagen"), (UInt32(90), "90 dagen"), (UInt32(0), "Altijd")], selection: $state.settings.historyRetentionDays)
                .frame(maxWidth: 220)
            }
            PaneSection(title: "Dicteerpad", subtitle: "Deze instelling geldt alleen voor het dicteerpad in Pennel. Fn-dicteren vanuit andere apps blijft direct invoegen.") {
                Toggle("Dicteerpad ook extern plakken", isOn: $state.settings.dictationPadAutoPaste)
                    .font(.figtree(12))
                Text("De tekst blijft in Pennel en wordt ook in de vastgelegde vorige app geplaatst.")
                    .font(.figtree(11)).foregroundStyle(Theme.muted)
                if !Permissions.accessibilityTrusted {
                    Text("Voor extern plakken is toegang tot Toegankelijkheid nodig.")
                        .font(.figtree(11)).foregroundStyle(Theme.muted)
                    Button("Geef toegang") { Permissions.promptAccessibility() }
                }
            }
            PaneSection(title: "Toegang", subtitle: "Controleer microfoon en toegang om tekst in andere apps te plakken.") {
                Button("Controleer toegang…") { state.onResumeOnboarding?() }
                    .buttonStyle(PennelControlButtonStyle())
            }
        }
        .onChange(of: state.settings.hotkey) { state.onHotkeyChange?(state.settings.hotkey) }
    }
}

struct DictionaryPane: View {
    @Bindable var state: AppState
    @State private var newTerm = ""

    var body: some View {
        PaneSection(title: "Woordenlijst", subtitle: "Namen en vaktermen die Pennel exact zo moet schrijven.") {
            HStack {
                TextField("Bv. Pennel, Kubernetes, Sarah", text: $newTerm)
                    .textFieldStyle(.roundedBorder)
                    .onSubmit(add)
                Button("Toevoegen", action: add).disabled(trimmed.isEmpty)
            }
            ForEach(state.settings.dictionary, id: \.self) { term in
                HStack {
                    Text(term)
                    Spacer()
                    Button {
                        state.settings.dictionary.removeAll { $0 == term }
                    } label: {
                        PennelIcon.close.image.foregroundStyle(Theme.muted).frame(width: 14, height: 14)
                    }
                    .buttonStyle(.plain).accessibilityLabel("Woord verwijderen")
                }
                .padding(.vertical, 2)
            }
        }
    }

    private var trimmed: String { newTerm.trimmingCharacters(in: .whitespaces) }

    private func add() {
        guard !trimmed.isEmpty, !state.settings.dictionary.contains(trimmed) else { return }
        state.settings.dictionary.append(trimmed)
        newTerm = ""
    }
}

extension Style {
    static let all: [Style] = [.neutral, .casual, .formal, .literal]

    var label: String {
        switch self {
        case .neutral: "Neutraal"
        case .casual: "Los"
        case .formal: "Netjes"
        case .literal: "Letterlijk"
        }
    }
}

struct StylesPane: View {
    @Bindable var state: AppState
    @State private var appId = ""

    var body: some View {
        PaneSection(title: "Stijl per app", subtitle: "Andere apps gebruiken Neutraal. Pennel kijkt enkel naar welke app actief is, nooit naar je scherm.") {
            ForEach($state.settings.styleRules, id: \.bundleId) { $rule in
                HStack(spacing: 10) {
                    AppIcon(bundleId: rule.bundleId)
                    Text(AppIcon.name(for: rule.bundleId)).lineLimit(1)
                    Spacer()
                    PennelPopup(label: "Stijl voor \(AppIcon.name(for: rule.bundleId))", options: Style.all.map { ($0, $0.label) }, selection: $rule.style)
                    .frame(width: 130)
                    Button {
                        state.settings.styleRules.removeAll { $0.bundleId == rule.bundleId }
                    } label: {
                        PennelIcon.close.image.foregroundStyle(Theme.muted).frame(width: 14, height: 14)
                    }
                    .buttonStyle(.plain).accessibilityLabel("Stijlregel verwijderen")
                }
                .font(.figtree(12)).padding(.vertical, 3)
            }
            Button("App toevoegen…", action: chooseApplication).buttonStyle(PennelControlButtonStyle())
            DisclosureGroup("App-ID handmatig toevoegen") {
                HStack(spacing: 10) {
                    TextField("App-id, bv. com.apple.TextEdit", text: $appId).textFieldStyle(.roundedBorder).onSubmit(addManual)
                    Button("Toevoegen", action: addManual).disabled(appId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }.padding(.top, 8)
            }
            .font(.figtree(12)).foregroundStyle(Theme.muted).padding(.top, 6)
        }
    }

    private func add(_ id: String) {
        guard !id.isEmpty, !state.settings.styleRules.contains(where: { $0.bundleId == id }) else { return }
        state.settings.styleRules.append(StyleRule(bundleId: id, style: .neutral))
    }

    private func addManual() {
        add(appId.trimmingCharacters(in: .whitespacesAndNewlines))
        appId = ""
    }

    private func chooseApplication() {
        let panel = NSOpenPanel()
        panel.title = "Kies een app"
        panel.prompt = "Toevoegen"
        panel.allowedContentTypes = [.application]
        panel.canChooseDirectories = false
        panel.canChooseFiles = true
        panel.allowsMultipleSelection = false
        panel.treatsFilePackagesAsDirectories = false
        guard panel.runModal() == .OK, let url = panel.url else { return }
        guard let id = Bundle(url: url)?.bundleIdentifier else {
            state.lastError = "Deze app heeft geen app-ID. Voeg het ID handmatig toe."
            return
        }
        add(id)
    }
}

struct AppIcon: View {
    let bundleId: String?

    var body: some View {
        PennelIcon.laptop.image.resizable()
            .frame(width: 18, height: 18)
            .foregroundStyle(Theme.muted)
    }

    static func url(for bundleId: String?) -> URL? {
        bundleId.flatMap { NSWorkspace.shared.urlForApplication(withBundleIdentifier: $0) }
    }

    static func icon(for bundleId: String?) -> NSImage {
        url(for: bundleId).map { NSWorkspace.shared.icon(forFile: $0.path) }
            ?? NSImage(systemSymbolName: "app", accessibilityDescription: nil)!
    }

    static func name(for bundleId: String) -> String {
        if bundleId.range(of: "pennel|musli", options: [.regularExpression, .caseInsensitive]) != nil { return "Pennel" }
        let known = ["com.apple.mail": "Mail", "com.apple.MobileSMS": "Berichten", "com.apple.Notes": "Notities", "com.apple.Terminal": "Terminal", "com.apple.Safari": "Safari", "com.google.Chrome": "Google Chrome", "com.microsoft.VSCode": "Visual Studio Code", "com.microsoft.Outlook": "Outlook", "com.tinyspeck.slackmacgap": "Slack", "com.anthropic.claudefordesktop": "Claude", "com.openai.chat": "ChatGPT", "com.microsoft.teams2": "Microsoft Teams", "us.zoom.xos": "Zoom", "com.apple.FaceTime": "FaceTime", "net.whatsapp.WhatsApp": "WhatsApp", "Cisco-Systems.Spark": "Webex", "com.hnc.Discord": "Discord", "company.thebrowser.Browser": "Arc", "com.microsoft.edgemac": "Microsoft Edge", "org.mozilla.firefox": "Firefox", "com.brave.Browser": "Brave", "ru.keepcoder.Telegram": "Telegram", "com.microsoft.Word": "Word", "com.googlecode.iterm2": "iTerm", "com.mitchellh.ghostty": "Ghostty", "dev.zed.Zed": "Zed", "com.todesktop.230313mzl4w4u92": "Cursor"]
        if let name = known[bundleId] { return name }
        let withoutExtension = bundleId.lowercased().hasSuffix(".exe") ? String(bundleId.dropLast(4)) : bundleId
        let name = withoutExtension.split(separator: ".").last.map(String.init) ?? bundleId
        return name.prefix(1).uppercased(with: Locale(identifier: "nl_BE")) + name.dropFirst()
    }
}
