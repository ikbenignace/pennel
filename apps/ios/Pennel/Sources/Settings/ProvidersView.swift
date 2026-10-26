import PennelMobileKit
import SwiftUI

struct ProviderRoute: Hashable {
    let id: String
}

struct ProvidersView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        List {
            Section {
                ForEach(model.settings.providers, id: \.id) { provider in
                    let reason = ProviderKinds.unavailableReason(provider.kind)
                    NavigationLink(value: ProviderRoute(id: provider.id)) {
                        VStack(alignment: .leading, spacing: Brand.Spacing.s1) {
                            Text(provider.name)
                                .foregroundStyle(reason == nil ? Brand.foreground.color : Brand.disabledText.color)
                            Text(subtitle(provider, reason: reason))
                                .font(Theme.font(.footnote))
                                .foregroundStyle(Brand.muted.color)
                        }
                    }
                }
            }
            .pennelRow()
            Section {
                Menu("Provider toevoegen…") {
                    ForEach(addableKinds, id: \.kind) { item in
                        let reason = ProviderKinds.unavailableReason(item.kind)
                        Button(reason.map { "\(item.label) — \($0)" } ?? item.label) { add(item.kind) }
                            .disabled(reason != nil)
                    }
                }
            }
            .pennelRow()
        }
        .pennelList()
    }

    private var addableKinds: [(kind: ProviderKind, label: String)] {
        ProviderKinds.all.filter { item in
            !(ProviderKinds.isSingleton(item.kind) && model.settings.providers.contains { $0.kind == item.kind })
        }
    }

    private func subtitle(_ provider: ProviderAccount, reason: String?) -> String {
        let label = ProviderKinds.label(provider.kind)
        if let reason { return "\(label) — \(reason)" }
        return "\(label) · \(isReady(provider) ? "Ingesteld" : "Nog instellen")"
    }

    private func isReady(_ provider: ProviderAccount) -> Bool {
        if provider.kind == .pennelCloud { return model.engine.cloudSignedIn() }
        return !requiresKey(kind: provider.kind) || model.hasKey(for: provider.id)
    }

    private func add(_ kind: ProviderKind) {
        model.update { settings in
            settings.providers.append(newAccount(kind: kind, existing: settings.providers))
        }
    }
}

struct ProviderDetailView: View {
    @Environment(AppModel.self) private var model
    @Environment(\.dismiss) private var dismiss
    let providerId: String

    @State private var name = ""
    @State private var baseURL = ""
    @State private var key = ""
    @State private var status: String?
    @State private var testing = false

    private var account: ProviderAccount? {
        model.settings.providers.first { $0.id == providerId }
    }

    var body: some View {
        Form {
            if let account {
                content(account)
            }
        }
        .pennelList()
        .navigationTitle(account.map { ProviderKinds.label($0.kind) } ?? "")
        .navigationBarTitleDisplayMode(.inline)
        .onAppear {
            name = account?.name ?? ""
            baseURL = account?.baseUrl ?? ""
        }
        .onDisappear(perform: saveFields)
    }

    @ViewBuilder
    private func content(_ account: ProviderAccount) -> some View {
        let reason = ProviderKinds.unavailableReason(account.kind)
        Section {
            TextField("Naam", text: $name)
                .onSubmit(saveFields)
            if ProviderKinds.usesBaseURL(account.kind) {
                TextField("Base URL", text: $baseURL, prompt: Text(baseURLPlaceholder(account.kind)))
                    .keyboardType(.URL)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .onSubmit(saveFields)
            }
            if requiresKey(kind: account.kind) {
                HStack {
                    SecureField("API-sleutel", text: $key, prompt: Text(model.hasKey(for: account.id) ? "•••••••• (bewaard)" : "Plak je sleutel"))
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .onSubmit(saveKey)
                    if model.hasKey(for: account.id) {
                        Button("Wis") { model.setKey(nil, for: account.id) }
                            .buttonStyle(.borderless)
                    }
                }
            }
        } footer: {
            if let reason { Text(reason) }
        }
        .pennelRow()
        .disabled(reason != nil)

        if reason == nil {
            actions(account)
        }

        Section {
            Button("Provider verwijderen", role: .destructive) {
                model.update { $0.providers.removeAll { $0.id == account.id } }
                model.setKey(nil, for: account.id)
                dismiss()
            }
            .disabled(model.settings.providers.count < 2)
        } footer: {
            Text("Kies bij Modellen welke provider je audio herkent en welke je tekst verwerkt. Daar kun je beide taken apart testen.")
        }
        .pennelRow()
    }

    @ViewBuilder
    private func actions(_ account: ProviderAccount) -> some View {
        switch account.kind {
        case .appleSpeech:
            Section {
                Button("Gebruik voor dicteren en meetings") {
                    let model = defaultModel(kind: account.kind, task: .dictationStt)
                    self.model.update { settings in
                        settings.dictationStt = ModelChoice(providerId: account.id, model: model)
                        settings.meetingStt = ModelChoice(providerId: account.id, model: model)
                    }
                }
            }
            .pennelRow()
        case .appleIntelligence:
            Section {
                Button("Gebruik voor tekstverwerking") {
                    let model = defaultModel(kind: account.kind, task: .llm)
                    self.model.update { $0.llm = ModelChoice(providerId: account.id, model: model) }
                }
            }
            .pennelRow()
        case .pennelCloud:
            CloudProviderCard()
        default:
            Section {
                Button(testing ? "Testen…" : "Test verbinding") { test(account) }
                    .disabled(testing)
            } footer: {
                if let status { Text(status) }
            }
            .pennelRow()
        }
    }

    private func baseURLPlaceholder(_ kind: ProviderKind) -> String {
        let url = defaultBaseUrl(kind: kind)
        return url.isEmpty ? "https://…/v1" : url
    }

    private func saveFields() {
        guard let account else { return }
        let name = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let baseURL = baseURL.trimmingCharacters(in: .whitespacesAndNewlines)
        model.update { settings in
            guard let index = settings.providers.firstIndex(where: { $0.id == account.id }) else { return }
            if !name.isEmpty { settings.providers[index].name = name }
            if ProviderKinds.usesBaseURL(account.kind) { settings.providers[index].baseUrl = baseURL }
        }
        saveKey()
    }

    private func saveKey() {
        guard account != nil, !key.isEmpty else { return }
        model.setKey(key, for: providerId)
        key = ""
    }

    private func test(_ account: ProviderAccount) {
        saveFields()
        testing = true
        status = "Testen…"
        let baseURL = model.settings.providers.first { $0.id == account.id }?.baseUrl ?? account.baseUrl
        let apiKey = Keychain.apiKeys().byProvider[account.id]
        Task {
            let outcome = await Task.detached { () -> String in
                do {
                    let models = try providerModels(baseUrl: baseURL, apiKey: apiKey)
                    return "Verbinding getest · \(models.count) modellen gevonden. Spraak en tekst zijn hiermee nog niet getest."
                } catch {
                    return ErrorText.failed(error)
                }
            }.value
            status = outcome
            testing = false
        }
    }
}

/// The Pennel Cloud card: who is signed in, or the way to the Account screen.
private struct CloudProviderCard: View {
    @Environment(AppModel.self) private var model
    private enum CardState { case loading, signedIn(String), signedOut, failed(String) }
    @State private var state = CardState.loading

    var body: some View {
        Section {
            switch state {
            case .loading:
                ProgressView()
            case .signedIn(let email):
                Text(email.isEmpty ? "Ingelogd" : "Ingelogd als \(email)")
            case .signedOut:
                Text("Log in bij Pennel Cloud")
                NavigationLink(value: SettingsSection.account) { Text("Log in bij Pennel Cloud") }
            case .failed(let message):
                Text(message).foregroundStyle(Brand.muted.color)
            }
        }
        .pennelRow()
        .task {
            let engine = model.engine
            state = await Task.detached { () -> State in
                do { return .signedIn(try engine.cloudAccount().email) }
                catch PennelError.NotSignedIn { return .signedOut }
                catch { return .failed(ErrorText.failed(error)) }
            }.value
        }
    }
}
