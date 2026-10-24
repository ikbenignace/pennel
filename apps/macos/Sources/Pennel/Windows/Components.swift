import PennelKit
import SwiftUI

struct SegmentTabs<Tab: Hashable>: View {
    let tabs: [(Tab, String)]
    @Binding var selection: Tab

    var body: some View {
        HStack(spacing: 2) {
            ForEach(tabs, id: \.0) { tab, title in
                Text(title)
                    .font(.figtree(12, weight: selection == tab ? .medium : .regular))
                    .foregroundStyle(selection == tab ? Theme.ink : Theme.muted)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 5)
                    .background(
                        RoundedRectangle(cornerRadius: 6)
                            .fill(selection == tab ? Theme.paper : .clear)
                            .shadow(color: .black.opacity(selection == tab ? 0.06 : 0), radius: 1, y: 1)
                    )
                    .contentShape(Rectangle())
                    .onTapGesture { selection = tab }
            }
        }
        .padding(2)
        .background(RoundedRectangle(cornerRadius: 8).fill(Theme.background))
    }
}

struct PaneSection<Content: View>: View {
    let title: String
    var subtitle: String?
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.figtree(13, weight: .semibold))
                if let subtitle {
                    Text(subtitle).font(.figtree(12)).foregroundStyle(Theme.muted)
                }
            }
            content
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: 12).fill(Theme.background))
    }
}

struct FieldRow<Content: View>: View {
    let label: String
    @ViewBuilder let content: Content

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 12) {
            Text(label)
                .font(.figtree(12))
                .foregroundStyle(Theme.muted)
                .frame(width: 90, alignment: .leading)
            content
        }
    }
}

struct StatusDot: View {
    let ok: Bool

    var body: some View {
        Circle()
            .fill(ok ? Color.green.opacity(0.7) : Theme.accent)
            .frame(width: 7, height: 7)
    }
}
