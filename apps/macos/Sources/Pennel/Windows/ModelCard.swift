@preconcurrency import PennelCore
import PennelKit
import SwiftUI

struct ModelCard<Actions: View>: View {
    let model: LocalModel
    let inUse: Bool
    let progress: Double?
    let error: String?
    @ViewBuilder let actions: Actions
    @State private var showLanguages = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(alignment: .firstTextBaseline) {
                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 6) {
                        Text(model.name).font(.figtree(13, weight: .medium))
                        if inUse { Badge(text: "In gebruik") } else if model.recommended { Badge(text: "Aanbevolen") }
                    }
                    Text(details).font(.figtree(11)).foregroundStyle(Theme.muted)
                }
                Spacer()
                actions
            }
            HStack(spacing: 16) {
                ScoreBar(label: "Snelheid", value: Int(model.speed))
                ScoreBar(label: "Nauwkeurigheid", value: model.accuracy.map(Int.init))
            }
            if !model.languages.isEmpty {
                Button(showLanguages ? "Verberg talen" : languageSummary) { showLanguages.toggle() }
                    .buttonStyle(.plain)
                    .font(.figtree(11))
                    .foregroundStyle(Theme.muted)
                if showLanguages {
                    Text(LanguageNames.list(model.languages))
                        .font(.figtree(11))
                        .foregroundStyle(Theme.ink)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            if let progress { ProgressView(value: progress).tint(Theme.accent) }
            if let error { Text(error).font(.figtree(11)).foregroundStyle(Theme.accent) }
        }
        .padding(.vertical, 6)
    }

    private var details: String {
        let size = "\(model.sizeBytes / 1_000_000) MB"
        return [model.description, size, model.parameters.map { "\($0) parameters" }]
            .compactMap { $0 }
            .joined(separator: " · ")
    }

    private var languageSummary: String {
        model.languages.count == 1
            ? "Alleen \(LanguageNames.name(model.languages[0]))"
            : "\(model.languages.count) talen ›"
    }
}

private struct Badge: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.figtree(11, weight: .medium))
            .foregroundStyle(Theme.accent)
            .padding(.horizontal, 6)
            .padding(.vertical, 1)
            .background(Capsule().stroke(Theme.accent.opacity(0.5)))
    }
}

private struct ScoreBar: View {
    let label: String
    let value: Int?

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack {
                Text(label)
                Spacer()
                Text(value.map { "\($0)" } ?? "niet gemeten")
            }
            .font(.figtree(11))
            .foregroundStyle(Theme.muted)
            GeometryReader { geometry in
                ZStack(alignment: .leading) {
                    Capsule().fill(Theme.line)
                    Capsule()
                        .fill(Theme.accent.opacity(0.8))
                        .frame(width: geometry.size.width * CGFloat(value ?? 0) / 100)
                }
            }
            .frame(height: 4)
        }
        .frame(maxWidth: 160)
    }
}
