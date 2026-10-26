import PennelMobileKit
import SwiftUI

/// Labels and icons of the desktop's sync badges (apps/desktop/src/sync-badge.ts).
enum SyncBadges {
    static func badge(for state: SyncItemState?, syncOn: Bool) -> (icon: String, label: String)? {
        guard syncOn, let state else { return nil }
        switch state {
        case .synced: return ("checkmark", "Gesynchroniseerd")
        case .pending: return ("arrow.triangle.2.circlepath", "Wacht op sync")
        case .conflict: return ("exclamationmark.triangle", "Conflictversie")
        }
    }

    /// The setting can stay on after logout, account deletion or a revoked device; the report tells whether sync can still happen.
    static func active(settingOn: Bool, status: SyncStatus?) -> Bool {
        switch status {
        case .off?, .signedOut?: false
        default: settingOn
        }
    }
}

struct SyncBadge: View {
    let state: SyncItemState?
    let syncOn: Bool

    var body: some View {
        if let badge = SyncBadges.badge(for: state, syncOn: syncOn) {
            Image(systemName: badge.icon)
                .font(Theme.font(.footnote))
                .foregroundStyle(state == .conflict ? Brand.accent.color : Brand.muted.color)
                .accessibilityLabel(badge.label)
        }
    }
}

/// Files another device saved next to a meeting file after a concurrent edit: `name (conflict …).ext`.
enum ConflictCopies {
    static func isConflictCopy(_ name: String) -> Bool {
        guard !name.isEmpty, !name.contains(where: { "/\\:\0".contains($0) }), name != ".", name != ".." else { return false }
        return (name as NSString).deletingPathExtension.contains(" (conflict ")
    }

    static func list(in directory: URL) -> [String] {
        let entries = (try? FileManager.default.contentsOfDirectory(
            at: directory, includingPropertiesForKeys: [.isRegularFileKey])) ?? []
        return entries
            .filter { (try? $0.resourceValues(forKeys: [.isRegularFileKey]).isRegularFile) == true }
            .map(\.lastPathComponent)
            .filter(isConflictCopy)
            .sorted()
    }

    static func remove(_ name: String, in directory: URL) throws {
        guard isConflictCopy(name) else { throw CocoaError(.fileWriteInvalidFileName) }
        let file = directory.appendingPathComponent(name)
        let regular = (try? file.resourceValues(forKeys: [.isRegularFileKey]).isRegularFile) == true
        guard regular else { throw CocoaError(.fileNoSuchFile) }
        try FileManager.default.removeItem(at: file)
    }
}

struct ConflictBanner: View {
    let name: String
    let directory: URL
    let removable: Bool
    let removed: () -> Void
    @State private var viewing = false
    @State private var confirming = false
    @State private var error: String?

    var body: some View {
        VStack(alignment: .leading, spacing: Brand.Spacing.s2) {
            Label("Er is een conflictversie van dit bestand.", systemImage: "exclamationmark.triangle")
                .foregroundStyle(Brand.accent.color)
            HStack {
                Button("Bekijken") { viewing = true }
                Button("Verwijderen", role: .destructive) { confirming = true }.disabled(!removable)
            }
            .buttonStyle(.borderless)
            if let error { Text(error).font(Theme.font(.footnote)).foregroundStyle(.red) }
        }
        .font(Theme.font(.footnote))
        .accessibilityElement(children: .contain)
        .sheet(isPresented: $viewing) {
            NavigationStack {
                ScrollView {
                    Text((try? String(contentsOf: directory.appendingPathComponent(name), encoding: .utf8)) ?? name)
                        .padding(Brand.Spacing.s4)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                .background(Brand.surface.color)
                .navigationTitle(name)
                .navigationBarTitleDisplayMode(.inline)
                .toolbar { Button("Sluit") { viewing = false } }
            }
        }
        .alert("Conflictversie verwijderen?", isPresented: $confirming) {
            Button("Annuleren", role: .cancel) {}
            Button("Verwijderen", role: .destructive) {
                do { try ConflictCopies.remove(name, in: directory); removed() } catch { self.error = ErrorText.failed(error) }
            }
        } message: {
            Text("\(name) wordt verwijderd.")
        }
    }
}
