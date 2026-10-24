import MusliKit
import Observation
import SwiftUI

@MainActor
@Observable
final class PillModel {
    var state: PillState = .hidden
    var level: Float = 0
}

struct PillView: View {
    let model: PillModel

    var body: some View {
        HStack(spacing: 10) {
            switch model.state {
            case .listening, .handsFree, .command:
                Wave(level: model.level)
            case .working:
                ProgressView().controlSize(.small).tint(Theme.wave)
            case .error:
                Circle().fill(Theme.accent).frame(width: 7, height: 7)
            case .hidden:
                EmptyView()
            }
            Text(label)
                .font(.figtree(12, weight: .medium))
                .foregroundStyle(Theme.paper)
                .fixedSize()
        }
        .padding(.leading, 12)
        .padding(.trailing, 16)
        .padding(.vertical, 8)
        .background(Capsule().fill(Theme.ink))
        .shadow(color: .black.opacity(0.25), radius: 12, y: 6)
        .padding(16)
    }

    private var label: String {
        switch model.state {
        case .hidden: ""
        case .listening: "Luistert…"
        case .handsFree: "Handenvrij — tik om te stoppen"
        case .command: "Opdracht…"
        case .working: "Bezig…"
        case .error(let message): message
        }
    }
}

private struct Wave: View {
    let level: Float
    private let weights: [CGFloat] = [0.5, 0.8, 1, 0.75, 0.45]

    var body: some View {
        HStack(alignment: .center, spacing: 2.5) {
            ForEach(weights.indices, id: \.self) { i in
                Capsule()
                    .fill(Theme.wave)
                    .frame(width: 2.5, height: 4 + 12 * weights[i] * min(1, CGFloat(level) * 12))
            }
        }
        .frame(height: 16)
        .animation(.easeOut(duration: 0.12), value: level)
    }
}
