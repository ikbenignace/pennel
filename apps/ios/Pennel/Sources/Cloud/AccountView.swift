import PennelMobileKit
import SwiftUI

/// The Account tab of Cloud v1, section for section as `apps/desktop/src/views/account.ts`.
struct AccountView: View {
    @Environment(AppModel.self) private var app
    @State private var model: CloudAccountModel?

    var body: some View {
        Group {
            if let model {
                AccountForm(account: model)
            } else {
                ProgressView()
            }
        }
        .background(Brand.surface.color)
        .task {
            guard model == nil else { return }
            let app = app
            let created = CloudAccountModel(
                service: app.engine, runner: .shared,
                settings: { app.settings }, settingsChanged: { app.reloadSettings() },
                openURL: { UIApplication.shared.open($0) })
            model = created
            await created.refresh()
        }
    }
}

private enum Confirmation: Identifiable {
    case logout, newCode, resetVault, deleteAccount, deletes
    case revoke(CloudDevice)

    var id: String {
        switch self {
        case .logout: "logout"
        case .newCode: "newCode"
        case .resetVault: "resetVault"
        case .deleteAccount: "deleteAccount"
        case .deletes: "deletes"
        case .revoke(let device): "revoke-\(device.id)"
        }
    }

    var title: String {
        switch self {
        case .logout: "Uitloggen?"
        case .newCode: "Nieuwe herstelcode maken?"
        case .resetVault: "Kluis opnieuw beginnen?"
        case .deleteAccount: "Account verwijderen?"
        case .deletes: "Verwijderingen bevestigen?"
        case .revoke: "Toestel afmelden?"
        }
    }

    var message: String {
        switch self {
        case .logout: "Je hebt je herstelcode nog niet bevestigd. Na uitloggen heb je hem nodig om je meetings terug te krijgen. Toch uitloggen?"
        case .newCode: "De oude herstelcode werkt daarna niet meer."
        case .resetVault: "Dit wist alle meetings in Pennel Cloud. Meetings op je toestellen blijven bestaan en worden opnieuw geüpload. Andere toestellen hebben daarna de nieuwe herstelcode nodig."
        case .deleteAccount: "Je account en alle versleutelde data in Pennel Cloud worden gewist. Meetings op dit toestel blijven staan."
        case .deletes: "Pennel verwijdert deze meetings dan ook in Pennel Cloud en op je andere toestellen. Ze blijven 30 dagen in de prullenbak op elk toestel."
        case .revoke: "Dit toestel kan dan niet meer synchroniseren. Wat het al had, blijft erop staan. Vrees je dat het gestolen is, begin dan de kluis opnieuw."
        }
    }

    var action: String {
        switch self {
        case .logout: "Uitloggen"
        case .newCode: "Nieuwe herstelcode maken"
        case .resetVault: "Kluis opnieuw beginnen"
        case .deleteAccount: "Account verwijderen"
        case .deletes: "Verwijderingen bevestigen"
        case .revoke: "Afmelden"
        }
    }
}

private struct AccountForm: View {
    @Environment(AppModel.self) private var app
    @Bindable var account: CloudAccountModel
    @State private var runner = CloudSyncRunner.shared
    @State private var confirmation: Confirmation?

    var body: some View {
        Form {
            if account.signedOut {
                signedOut
            } else {
                if account.offerCloud { cloudPrompt }
                accountSection
                if let usage = account.usage { usageSection(usage) }
                syncSection
                if let code = account.shownCode {
                    RecoveryCodeView(code: code) { account.codeSaved() }
                }
                if account.unlockVisible { unlockSection }
                if account.syncEnabled, account.shownCode == nil, !account.needsVault { recoverySection }
                if let devices = account.devices { devicesSection(devices) }
                deleteSection
            }
        }
        .pennelList()
        .onChange(of: runner.report) { _, report in
            guard let report else { return }
            account.adopt(report)
        }
        .alert(confirmation?.title ?? "", isPresented: Binding(get: { confirmation != nil }, set: { if !$0 { confirmation = nil } }),
               presenting: confirmation) { item in
            Button("Annuleren", role: .cancel) {}
            Button(item.action, role: item.isDestructive ? .destructive : nil) { confirm(item) }
        } message: { item in
            Text(item.message)
        }
    }

    private func confirm(_ item: Confirmation) {
        Task {
            switch item {
            case .logout: await account.logout()
            case .newCode: await account.newRecoveryCode()
            case .resetVault: await account.resetVault()
            case .deleteAccount: await account.deleteAccount()
            case .deletes: await account.confirmDeletes()
            case .revoke(let device): await account.revoke(device)
            }
        }
    }

    // MARK: Uitgelogd

    @ViewBuilder private var signedOut: some View {
        switch account.login {
        case .code(let start):
            Section {
                Text("Je browser is geopend. Controleer daar deze code:")
                Text(start.userCode)
                    .font(.system(.title, design: .monospaced, weight: .semibold))
                    .accessibilityLabel("Code")
                Button("Browser opnieuw openen") { account.reopenBrowser() }
                Button("Annuleren") { account.cancelLogin() }
            }
            .pennelRow()
        case .expired:
            Section {
                Text("De code is verlopen.").foregroundStyle(.red)
                Button("Opnieuw beginnen") { Task { await account.beginLogin() } }
            }
            .pennelRow()
        case .idle:
            Section {
                Text("Log in om je meetings op al je toestellen te hebben en Pennel Cloud als provider te gebruiken, zonder eigen API-sleutel.")
                TextField("Naam van dit toestel", text: $account.deviceName)
                    .textInputAutocapitalization(.words)
                    .submitLabel(.done)
                if let notice = account.notice { Text(notice).foregroundStyle(.red) }
                Button("Inloggen") { Task { await account.beginLogin() } }
            }
            .pennelRow()
        }
    }

    // MARK: Aanbod

    private var emptySlots: [(label: String, task: ProviderTask)] {
        let settings = app.settings
        let known = Set(settings.providers.map(\.id))
        var slots: [(label: String, task: ProviderTask)] = []
        if !known.contains(settings.dictationStt.providerId) { slots.append(("dicteren", .dictationStt)) }
        if !known.contains(settings.llm.providerId) { slots.append(("opkuisen en samenvatten", .llm)) }
        if !known.contains(settings.meetingStt.providerId) { slots.append(("meetings", .meetingStt)) }
        return slots
    }

    @ViewBuilder private var cloudPrompt: some View {
        let empty = emptySlots
        if !empty.isEmpty {
            Section {
                Text("Pennel Cloud gebruiken voor \(Self.list(empty.map(\.label)))?")
                Button("Ja") { adopt(empty.map(\.task)) }
                Button("Nee") { account.offerCloud = false }
            }
            .pennelRow()
        }
    }

    private static func list(_ items: [String]) -> String {
        items.count < 2 ? items.joined() : "\(items.dropLast().joined(separator: ", ")) en \(items.last ?? "")"
    }

    private func adopt(_ tasks: [ProviderTask]) {
        account.offerCloud = false
        app.update { settings in
            var cloud = settings.providers.first { $0.kind == .pennelCloud }
            if cloud == nil {
                let created = newAccount(kind: .pennelCloud, existing: settings.providers)
                settings.providers.append(created)
                cloud = created
            }
            guard let id = cloud?.id else { return }
            for task in tasks {
                let choice = ModelChoice(providerId: id, model: defaultModel(kind: .pennelCloud, task: task))
                switch task {
                case .dictationStt: settings.dictationStt = choice
                case .meetingStt: settings.meetingStt = choice
                case .llm: settings.llm = choice
                }
            }
        }
    }

    // MARK: Account en gebruik

    private var accountSection: some View {
        Section("Account") {
            if let info = account.account {
                Text("Ingelogd als \(info.email)")
            } else if let error = account.accountError {
                Text(error).foregroundStyle(.red)
            }
            Button("Uitloggen") {
                if account.recoveryUnconfirmed { confirmation = .logout } else { Task { await account.logout() } }
            }
        }
        .pennelRow()
    }

    private func usageSection(_ usage: CloudFormat.Usage) -> some View {
        Section("Gebruik deze maand") {
            Text(usage.audio)
            Text(usage.tokens)
            Text(usage.storage)
        }
        .pennelRow()
    }

    // MARK: Sync

    private var statusLine: String? {
        if account.syncing { return "Bezig…" }
        guard account.syncEnabled else { return nil }
        let report = account.report
        switch report.status {
        case .pending: return "Wacht op eerste sync…"
        case .synced:
            guard let last = report.lastSuccessMs else { return "Gesynchroniseerd" }
            return "Gesynchroniseerd · \(CloudFormat.relativeTime(thenMs: last, nowMs: Int64(Date().timeIntervalSince1970 * 1000)))"
        case .offline: return "Offline — probeert later opnieuw"
        case .storageFull(let message), .error(let message): return message
        default: return nil
        }
    }

    private var statusIsError: Bool {
        if account.braked { return true }
        switch account.report.status {
        case .error, .storageFull: return true
        default: return false
        }
    }

    private var syncSection: some View {
        let setup = account.syncEnabled && account.report.status == .needsSetup && account.shownCode == nil
        return Section {
            Toggle("Meetings synchroniseren", isOn: Binding(
                get: { account.syncEnabled },
                set: { on in Task { await account.setSync(on) } }))
            if let statusLine {
                Text(statusLine)
                    .font(Theme.font(.footnote))
                    .foregroundStyle(statusIsError ? Color.red : Brand.muted.color)
                    .accessibilityAddTraits(.updatesFrequently)
            }
            if !account.unlockVisible, let notice = account.notice { Text(notice).foregroundStyle(.red) }
            if account.braked { Button("Verwijderingen bevestigen") { confirmation = .deletes } }
            if setup { Button("Herstelcode maken") { Task { await account.createCode() } } }
            if account.syncEnabled, !account.needsVault {
                Button("Nu synchroniseren") { account.syncNow() }.disabled(account.syncing)
            }
        } header: {
            Text("Meetings synchroniseren")
        } footer: {
            Text("End-to-end versleuteld: Pennel Cloud kan je meetings niet lezen. Audio blijft op dit toestel.")
        }
        .pennelRow()
    }

    // MARK: Herstelcode

    private var unlockSection: some View {
        Section("Herstelcode") {
            if account.syncEnabled, account.report.status == .vaultReset {
                Text("De kluis is opnieuw begonnen op een ander toestel. Voer de nieuwe herstelcode in.")
            }
            TextField("Herstelcode", text: $account.codeDraft)
                .textInputAutocapitalization(.characters)
                .autocorrectionDisabled()
                .submitLabel(.go)
                .onSubmit { Task { await account.unlock() } }
            Button("Ontgrendelen") { Task { await account.unlock() } }
            if let notice = account.notice { Text(notice).foregroundStyle(.red) }
            Button("Herstelcode kwijt?") { account.lostCode.toggle() }
            if account.lostCode {
                Text("Zonder herstelcode kan niemand de meetings in Pennel Cloud nog lezen. Je kunt de kluis opnieuw beginnen.")
                    .font(Theme.font(.footnote))
                Button("Kluis opnieuw beginnen", role: .destructive) { confirmation = .resetVault }
            }
        }
        .pennelRow()
    }

    private var recoverySection: some View {
        Section("Herstelcode") {
            if account.recoveryUnconfirmed {
                Text("Je hebt je herstelcode nog niet bevestigd.").foregroundStyle(.red)
            }
            Button("Nieuwe herstelcode maken") { confirmation = .newCode }
        }
        .pennelRow()
    }

    // MARK: Toestellen

    private func devicesSection(_ devices: [CloudDevice]) -> some View {
        Section("Toestellen") {
            ForEach(devices, id: \.id) { device in
                DeviceRow(device: device) { name in Task { await account.rename(device, to: name) } }
                if !device.current {
                    Button("Afmelden") { confirmation = .revoke(device) }.buttonStyle(.borderless)
                }
            }
        }
        .pennelRow()
    }

    private var deleteSection: some View {
        Section("Account verwijderen") {
            Button("Account verwijderen", role: .destructive) { confirmation = .deleteAccount }
        }
        .pennelRow()
    }
}

private extension Confirmation {
    var isDestructive: Bool {
        switch self {
        case .newCode, .deletes: false
        default: true
        }
    }
}

private struct DeviceRow: View {
    let device: CloudDevice
    let rename: (String) -> Void
    @State private var name = ""

    var body: some View {
        VStack(alignment: .leading, spacing: Brand.Spacing.s1) {
            TextField("Naam van toestel", text: $name)
                .onSubmit { rename(name) }
            Text("\(device.platform) · \(seen)\(device.current ? " (dit toestel)" : "")")
                .font(Theme.font(.footnote))
                .foregroundStyle(Brand.muted.color)
        }
        .onAppear { name = device.name }
    }

    private var seen: String {
        guard let date = MeetingDates.date(device.lastSeenIso) else { return "laatst gezien \(device.lastSeenIso)" }
        return "laatst gezien \(CloudFormat.relativeTime(thenMs: Int64(date.timeIntervalSince1970 * 1000), nowMs: Int64(Date().timeIntervalSince1970 * 1000)))"
    }
}
