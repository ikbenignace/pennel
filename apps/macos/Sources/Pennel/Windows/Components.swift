import PennelKit
import SwiftUI

struct SegmentTabs<Tab: Hashable>: View {
    let tabs: [(Tab, String)]
    @Binding var selection: Tab
    var disabledTabs: Set<Tab> = []
    @FocusState private var focusedTab: Tab?

    var body: some View {
        HStack(spacing: 2) {
            ForEach(tabs, id: \.0) { tab, title in
                Button { selection = tab } label: { Text(title) }
                    .buttonStyle(.plain)
                    .disabled(disabledTabs.contains(tab))
                    .font(.figtree(12, weight: selection == tab ? .medium : .regular))
                    .foregroundStyle(selection == tab ? Theme.ink : Theme.muted)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .frame(maxWidth: .infinity, minHeight: 36)
                    .background(
                        RoundedRectangle(cornerRadius: 6)
                            .fill(selection == tab ? Theme.field : .clear)
                    )
                    .contentShape(Rectangle())
                    .accessibilityLabel(title)
                    .accessibilityAddTraits(selection == tab ? .isSelected : [])
                    .focused($focusedTab, equals: tab)
                    .onKeyPress(.leftArrow) { move(-1); return .handled }
                    .onKeyPress(.rightArrow) { move(1); return .handled }
            }
        }
        .padding(3)
        .background(RoundedRectangle(cornerRadius: 8).fill(Theme.sidebar))
    }

    private func move(_ distance: Int) {
        let available = tabs.filter { !disabledTabs.contains($0.0) }
        guard !available.isEmpty else { return }
        let current = available.firstIndex(where: { $0.0 == selection }) ?? (distance > 0 ? available.count - 1 : 0)
        let next = available[(current + distance + available.count) % available.count].0
        selection = next
        focusedTab = next
    }
}

struct UnderlineTabs<Tab: Hashable>: View {
    let tabs: [(Tab, String)]
    @Binding var selection: Tab
    @FocusState private var focusedTab: Tab?
    var body: some View {
        HStack(spacing: 24) {
            ForEach(tabs, id: \.0) { tab, title in
                Button { selection = tab } label: {
                    Text(title).font(.figtree(12, weight: selection == tab ? .medium : .regular))
                        .foregroundStyle(selection == tab ? Theme.ink : Theme.muted)
                        .padding(.bottom, 11)
                        .overlay(alignment: .bottom) { Rectangle().fill(selection == tab ? Theme.ink : .clear).frame(height: 2) }
                }
                .buttonStyle(.plain).accessibilityAddTraits(selection == tab ? .isSelected : [])
                .focused($focusedTab, equals: tab)
                .onKeyPress(.leftArrow) { move(-1); return .handled }
                .onKeyPress(.rightArrow) { move(1); return .handled }
            }
            Spacer(minLength: 0)
        }
        .overlay(alignment: .bottom) { Rectangle().fill(Theme.line).frame(height: 1).offset(y: 1) }
    }

    private func move(_ distance: Int) {
        guard let current = tabs.firstIndex(where: { $0.0 == selection }), !tabs.isEmpty else { return }
        let next = tabs[(current + distance + tabs.count) % tabs.count].0
        selection = next
        focusedTab = next
    }
}

struct PennelControlButtonStyle: ButtonStyle {
    @Environment(\.isEnabled) private var enabled
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.font(.figtree(12, weight: .medium))
            .padding(.horizontal, 11).padding(.vertical, 6)
            .foregroundStyle(Theme.ink)
            .background(RoundedRectangle(cornerRadius: 6).fill(configuration.isPressed ? Theme.hover : Theme.field))
            .overlay(RoundedRectangle(cornerRadius: 6).stroke(Theme.line))
            .opacity(enabled ? 1 : 0.5)
    }
}

struct PennelPrimaryButtonStyle: ButtonStyle {
    @Environment(\.isEnabled) private var enabled
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.font(.figtree(12, weight: .medium))
            .padding(.horizontal, 11).padding(.vertical, 6)
            .foregroundStyle(Theme.buttonText)
            .background(RoundedRectangle(cornerRadius: 6).fill(Theme.button))
            .opacity(enabled ? (configuration.isPressed ? 0.88 : 1) : 0.45)
    }
}

struct PennelPopup<Value: Hashable>: View {
    let label: String
    let options: [(Value, String)]
    @Binding var selection: Value
    var disabledValues: Set<Value> = []
    var body: some View {
        Menu {
            ForEach(options, id: \.0) { value, title in
                Button(title) { selection = value }.disabled(disabledValues.contains(value))
            }
        } label: {
            HStack(spacing: 10) {
                Text(options.first { $0.0 == selection }?.1 ?? "")
                Spacer(minLength: 0)
                Image(systemName: "chevron.down").font(.system(size: 9, weight: .medium))
            }
            .font(.figtree(12)).foregroundStyle(Theme.ink)
            .padding(.horizontal, 9).padding(.vertical, 6)
            .background(RoundedRectangle(cornerRadius: 6).fill(Theme.field))
            .overlay(RoundedRectangle(cornerRadius: 6).stroke(Theme.line))
        }
        .menuStyle(.borderlessButton).menuIndicator(.hidden)
        .accessibilityLabel("\(label), \(options.first { $0.0 == selection }?.1 ?? "")")
    }
}

struct PaneSection<Content: View>: View {
    let title: String
    var subtitle: String?
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            VStack(alignment: .leading, spacing: 4) {
                Text(title).font(.figtree(13, weight: .semibold))
                if let subtitle {
                    Text(subtitle).font(.figtree(12)).foregroundStyle(Theme.muted)
                }
            }
            content
        }
        .padding(.bottom, 16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .overlay(alignment: .bottom) { Rectangle().fill(Theme.line).frame(height: 1) }
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
            .fill(ok ? Theme.muted : Theme.accent)
            .frame(width: 7, height: 7)
            .accessibilityLabel(ok ? "In orde" : "Nog instellen")
    }
}
