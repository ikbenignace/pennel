import AppKit
@preconcurrency import MusliCore
import MusliKit
import SwiftUI

extension Notification.Name {
    static let musliHistoryChanged = Notification.Name("dev.musli.historyChanged")
}

struct HistoryView: View {
    let state: AppState
    @State private var entries: [HistoryEntry] = []
    @State private var retrying: Set<String> = []
    @State private var message: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text("Geschiedenis")
                .font(.custom("Instrument Serif", size: 28))
                .padding(.horizontal, 24)
                .padding(.top, 28)
                .padding(.bottom, 12)
            if let message {
                Text(message).font(.figtree(12)).foregroundStyle(Theme.accent).padding(.horizontal, 24)
            }
            if entries.isEmpty {
                Text("Nog geen dictaten.")
                    .foregroundStyle(Theme.muted)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                List(entries, id: \.id) { entry in
                    row(entry).listRowSeparatorTint(Theme.line)
                }
                .scrollContentBackground(.hidden)
            }
        }
        .frame(minWidth: 520, minHeight: 420)
        .background(Theme.paper)
        .onAppear(perform: reload)
        .onReceive(NotificationCenter.default.publisher(for: .musliHistoryChanged)) { _ in reload() }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didBecomeKeyNotification)) { _ in reload() }
    }

    private func row(_ entry: HistoryEntry) -> some View {
        let failed = entry.status == .failed
        return HStack(alignment: .top, spacing: 10) {
            AppIcon(bundleId: entry.appBundleId).padding(.top, 2)
            VStack(alignment: .leading, spacing: 3) {
                Text(failed ? "Mislukt: \(entry.error ?? "onbekende fout")" : entry.text)
                    .foregroundStyle(failed ? Theme.muted : Theme.ink)
                    .textSelection(.enabled)
                Text(Date(timeIntervalSince1970: Double(entry.createdAtMs) / 1000).formatted(date: .abbreviated, time: .shortened))
                    .font(.figtree(11))
                    .foregroundStyle(Theme.muted)
            }
            Spacer()
            if failed && entry.hasAudio {
                Button(retrying.contains(entry.id) ? "Bezig…" : "Opnieuw proberen") { retry(entry) }
                    .disabled(retrying.contains(entry.id))
            } else if !failed {
                Button("Kopiëren") {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(entry.text, forType: .string)
                }
            }
            Button {
                try? state.engine.deleteHistory(id: entry.id)
                reload()
            } label: {
                Image(systemName: "trash").foregroundStyle(Theme.muted)
            }
            .buttonStyle(.plain)
        }
        .padding(.vertical, 6)
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
