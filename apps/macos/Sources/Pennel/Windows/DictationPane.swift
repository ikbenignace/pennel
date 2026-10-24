import AppKit
@preconcurrency import PennelCore
import PennelKit
import SwiftUI

struct DictationPane: View {
    @Bindable var state: AppState

    var body: some View {
        VStack(spacing: 14) {
            PaneSection(title: "Stand") {
                Picker("", selection: $state.settings.mode) {
                    Text("Opgekuist — euh's weg, zelfcorrecties verwerkt, stijl per app").tag(DictationMode.clean)
                    Text("Snel — letterlijk wat je zei, zonder taalmodel").tag(DictationMode.fast)
                }
                .pickerStyle(.radioGroup)
                .labelsHidden()
            }
            PaneSection(title: "Sneltoets", subtitle: "Ingedrukt houden om te praten, twee keer tikken voor handenvrij.") {
                Picker("", selection: $state.settings.hotkey) {
                    ForEach([HotkeyTrigger.fn, .rightOption, .rightCommand], id: \.self) { Text($0.label).tag($0) }
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .frame(maxWidth: 320)
            }
            PaneSection(title: "Taal") {
                Picker("", selection: $state.settings.language) {
                    Text("Automatisch").tag(String?.none)
                    Text("Nederlands").tag(String?.some("nl"))
                    Text("Engels").tag(String?.some("en"))
                }
                .labelsHidden()
                .frame(maxWidth: 220)
            }
            PaneSection(title: "Geschiedenis bewaren") {
                Picker("", selection: $state.settings.historyRetentionDays) {
                    Text("7 dagen").tag(UInt32(7))
                    Text("30 dagen").tag(UInt32(30))
                    Text("90 dagen").tag(UInt32(90))
                    Text("Altijd").tag(UInt32(0))
                }
                .labelsHidden()
                .frame(maxWidth: 220)
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
                        Image(systemName: "xmark").foregroundStyle(Theme.muted)
                    }
                    .buttonStyle(.plain)
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

    var body: some View {
        PaneSection(title: "Stijl per app", subtitle: "Andere apps gebruiken Neutraal. Pennel kijkt enkel naar welke app actief is, nooit naar je scherm.") {
            ForEach($state.settings.styleRules, id: \.bundleId) { $rule in
                HStack(spacing: 10) {
                    AppIcon(bundleId: rule.bundleId)
                    Text(AppIcon.name(for: rule.bundleId)).lineLimit(1)
                    Spacer()
                    Picker("", selection: $rule.style) {
                        ForEach(Style.all, id: \.self) { Text($0.label).tag($0) }
                    }
                    .labelsHidden()
                    .frame(width: 130)
                    Button {
                        state.settings.styleRules.removeAll { $0.bundleId == rule.bundleId }
                    } label: {
                        Image(systemName: "xmark").foregroundStyle(Theme.muted)
                    }
                    .buttonStyle(.plain)
                }
            }
            Menu("App toevoegen…") {
                ForEach(addableApps, id: \.bundleIdentifier) { app in
                    Button(app.localizedName ?? app.bundleIdentifier ?? "") {
                        guard let id = app.bundleIdentifier else { return }
                        state.settings.styleRules.append(StyleRule(bundleId: id, style: .neutral))
                    }
                }
            }
            .frame(maxWidth: 200)
        }
    }

    private var addableApps: [NSRunningApplication] {
        let known = Set(state.settings.styleRules.map(\.bundleId))
        return NSWorkspace.shared.runningApplications
            .filter { $0.activationPolicy == .regular && $0.bundleIdentifier.map { !known.contains($0) } ?? false }
            .sorted { ($0.localizedName ?? "") < ($1.localizedName ?? "") }
    }
}

struct AppIcon: View {
    let bundleId: String?

    var body: some View {
        Image(nsImage: Self.icon(for: bundleId))
            .resizable()
            .frame(width: 18, height: 18)
    }

    static func url(for bundleId: String?) -> URL? {
        bundleId.flatMap { NSWorkspace.shared.urlForApplication(withBundleIdentifier: $0) }
    }

    static func icon(for bundleId: String?) -> NSImage {
        url(for: bundleId).map { NSWorkspace.shared.icon(forFile: $0.path) }
            ?? NSImage(systemSymbolName: "app", accessibilityDescription: nil)!
    }

    static func name(for bundleId: String) -> String {
        url(for: bundleId).map { FileManager.default.displayName(atPath: $0.path).replacingOccurrences(of: ".app", with: "") }
            ?? bundleId
    }
}
