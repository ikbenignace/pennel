import AppKit
@preconcurrency import PennelCore
import PennelKit
import SwiftUI

extension Notification.Name {
    static let pennelHistoryChanged = Notification.Name("dev.pennel.historyChanged")
}

struct HistoryView: View {
    let state: AppState
    @State private var entries: [HistoryEntry] = []
    private var retrying: Set<String> {
        get { state.workspace.historyRetrying }
        nonmutating set { state.workspace.historyRetrying = newValue }
    }
    private var message: String? {
        get { state.workspace.historyMessage }
        nonmutating set { state.workspace.historyMessage = newValue }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Geschiedenis").font(.figtree(21, weight: .semibold)).padding(.bottom, 26)
            if let message { Text(message).font(.figtree(12)).foregroundStyle(Theme.accent).padding(.bottom, 12) }
            if entries.isEmpty {
                Text("Nog geen dictaten.").foregroundStyle(Theme.muted).frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 0) {
                        ForEach(entries, id: \.id) { entry in row(entry) }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
        .padding(.horizontal, 37).padding(.top, 32).padding(.bottom, 22)
        .frame(maxWidth: 850, maxHeight: .infinity, alignment: .topLeading)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Theme.paper)
        .onAppear(perform: reload)
        .onReceive(NotificationCenter.default.publisher(for: .pennelHistoryChanged)) { _ in reload() }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didBecomeKeyNotification)) { _ in reload() }
    }

    private func row(_ entry: HistoryEntry) -> some View {
        let failed = entry.status == .failed
        return HStack(alignment: .top, spacing: 10) {
            PennelIcon.laptop.image.resizable().scaledToFit().frame(width: 16, height: 16)
                .foregroundStyle(Theme.muted).padding(.top, 3)
            VStack(alignment: .leading, spacing: 5) {
                Text(failed ? "Mislukt: \(entry.error ?? "onbekende fout")" : entry.text)
                    .font(.figtree(14)).foregroundStyle(failed ? Theme.accent : Theme.ink).textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                Text(metadata(entry)).font(.figtree(11)).foregroundStyle(Theme.muted)
                if failed && entry.hasAudio {
                    Button(retrying.contains(entry.id) ? "Bezig…" : "Opnieuw proberen") { retry(entry) }
                        .disabled(retrying.contains(entry.id)).buttonStyle(.plain).font(.figtree(12))
                }
            }
            if !failed {
                Button {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(entry.text, forType: .string)
                } label: {
                    PennelIcon.copy.image.resizable().scaledToFit().frame(width: 14, height: 14)
                        .frame(width: 28, height: 28)
                }
                .buttonStyle(.plain).foregroundStyle(Theme.muted).help("Kopiëren").accessibilityLabel("Dictaat kopiëren")
            }
            Button { try? state.engine.deleteHistory(id: entry.id); reload() } label: {
                PennelIcon.trash.image.resizable().scaledToFit().frame(width: 14, height: 14)
                    .frame(width: 28, height: 28)
            }
            .buttonStyle(.plain).foregroundStyle(Theme.muted).help("Verwijder").accessibilityLabel("Dictaat verwijderen")
        }
        .padding(.vertical, 12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .overlay(alignment: .bottom) { Rectangle().fill(Theme.line).frame(height: 1) }
    }

    private func metadata(_ entry: HistoryEntry) -> String {
        let date = LibraryView.formattedDate(Date(timeIntervalSince1970: Double(entry.createdAtMs) / 1000))
        guard let app = entry.appBundleId, !app.isEmpty else { return date }
        return date + " · " + AppIcon.name(for: app)
    }

    private func reload() {
        entries = (try? state.engine.history(limit: 200)) ?? []
    }

    private func retry(_ entry: HistoryEntry) {
        retrying.insert(entry.id)
        let engine = state.engine
        let keys = state.keys()
        let id = entry.id
        Task {
            let outcome = await Task.detached { Result { try engine.retry(historyId: id, keys: keys) } }.value
            retrying.remove(id)
            if case .failure(let error) = outcome { message = "Opnieuw proberen mislukt: \(error)" } else { message = nil }
            reload()
        }
    }
}
