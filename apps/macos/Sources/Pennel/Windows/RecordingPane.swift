@preconcurrency import PennelCore
import PennelKit
import SwiftUI

extension RecordAction {
    static let all: [RecordAction] = [.ask, .always, .never]

    var label: String {
        switch self {
        case .ask: "Vragen"
        case .always: "Altijd"
        case .never: "Nooit"
        }
    }
}

struct RecordingPane: View {
    @Bindable var state: AppState
    let calendar: CalendarProvider
    @State private var calendars: [(id: String, name: String)] = []
    @State private var newEventTitle = ""

    var body: some View {
        VStack(spacing: 14) {
            PaneSection(title: "Calls herkennen", subtitle: "Pennel merkt wanneer een call-app je microfoon gebruikt en vraagt of je wilt opnemen.") {
                Toggle("Automatisch herkennen", isOn: $state.settings.autoDetect)
                Toggle("Sneltoets ⌘⇧R om een opname te starten of stoppen", isOn: $state.settings.recordShortcutEnabled)
                Toggle("Echo-onderdrukking (als je via speakers belt)", isOn: $state.settings.echoCancellation)
                Toggle("Toestemmingszin kopiëren bij de start van een opname", isOn: $state.settings.consentEnabled)
            }
            PaneSection(title: "Per app") {
                ForEach(appIds, id: \.self) { id in
                    HStack(spacing: 10) {
                        AppIcon(bundleId: id)
                        Text(AppIcon.name(for: id)).lineLimit(1)
                        Spacer()
                        Picker("", selection: appAction(id)) {
                            ForEach(RecordAction.all, id: \.self) { Text($0.label).tag($0) }
                        }
                        .labelsHidden()
                        .frame(width: 110)
                    }
                }
            }
            PaneSection(title: "Agenda", subtitle: "Geeft meetings de titel en deelnemers uit je Mac-agenda. Outlook verschijnt hier als je het werkaccount toevoegt bij Systeeminstellingen → Internetaccounts.") {
                Toggle("Agenda gebruiken", isOn: $state.settings.calendarEnabled)
                    .onChange(of: state.settings.calendarEnabled) {
                        guard state.settings.calendarEnabled else { return }
                        Task {
                            _ = await calendar.requestAccess()
                            calendars = calendar.calendars()
                        }
                    }
                if state.settings.calendarEnabled {
                    ForEach(calendars, id: \.id) { item in
                        HStack {
                            Text(item.name).lineLimit(1)
                            Spacer()
                            Picker("", selection: calendarAction(item.id, item.name)) {
                                ForEach(RecordAction.all, id: \.self) { Text($0.label).tag($0) }
                            }
                            .labelsHidden()
                            .frame(width: 110)
                        }
                    }
                }
            }
            PaneSection(title: "Per meeting", subtitle: "Bijvoorbeeld je terugkerende standup altijd, of ‘Lunch’ nooit. Gaat voor op agenda en app.") {
                HStack {
                    TextField("Titel van de meeting", text: $newEventTitle).textFieldStyle(.roundedBorder)
                    Button("Toevoegen") {
                        let title = newEventTitle.trimmingCharacters(in: .whitespaces)
                        guard !title.isEmpty else { return }
                        state.settings.eventRules.append(EventRule(title: title, action: .always))
                        newEventTitle = ""
                    }
                }
                ForEach($state.settings.eventRules, id: \.title) { $rule in
                    HStack {
                        Text(rule.title)
                        Spacer()
                        Picker("", selection: $rule.action) {
                            ForEach(RecordAction.all, id: \.self) { Text($0.label).tag($0) }
                        }
                        .labelsHidden()
                        .frame(width: 110)
                        Button {
                            state.settings.eventRules.removeAll { $0.title == rule.title }
                        } label: {
                            Image(systemName: "xmark").foregroundStyle(Theme.muted)
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
        }
        .onAppear { calendars = calendar.calendars() }
        .onChange(of: state.settings.autoDetect) { state.onRecordingSettingsChange?() }
        .onChange(of: state.settings.recordShortcutEnabled) { state.onRecordingSettingsChange?() }
    }

    private var appIds: [String] {
        let known = knownCallApps() + browserApps()
        let extra = state.settings.appRules.map(\.bundleId).filter { !known.contains($0) }
        return (known + extra).filter { AppIcon.url(for: $0) != nil || extra.contains($0) }
    }

    private func appAction(_ id: String) -> Binding<RecordAction> {
        Binding(
            get: { state.settings.appRules.first { $0.bundleId == id }?.action ?? .ask },
            set: { action in
                state.settings.appRules.removeAll { $0.bundleId == id }
                if action != .ask { state.settings.appRules.append(AppRule(bundleId: id, action: action)) }
            }
        )
    }

    private func calendarAction(_ id: String, _ name: String) -> Binding<RecordAction> {
        Binding(
            get: { state.settings.calendarRules.first { $0.calendarId == id }?.action ?? .ask },
            set: { action in
                state.settings.calendarRules.removeAll { $0.calendarId == id }
                if action != .ask {
                    state.settings.calendarRules.append(CalendarRule(calendarId: id, calendarName: name, action: action))
                }
            }
        )
    }
}
