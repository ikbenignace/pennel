import PennelMobileKit
import SwiftUI

struct KeyboardRoot: View {
    let model: KeyboardModel
    let layout: KeyboardLayout
    let showsGlobe: Bool
    let advance: () -> Void
    let openSettings: () -> Void
    let insert: (String) -> Void
    let delete: () -> Void
    @State private var showsLetters = false

    var body: some View {
        VStack(spacing: Brand.Spacing.s2) {
            if showsLetters {
                LetterPad(layout: layout, insert: insert, delete: delete)
            } else {
                DictationPad(model: model, openSettings: openSettings)
                    .frame(maxHeight: .infinity)
            }
            bottomRow
        }
        .padding(.horizontal, Brand.Spacing.s2)
        .padding(.vertical, Brand.Spacing.s2)
        .background(Brand.surface.color)
    }

    private var bottomRow: some View {
        HStack(spacing: 5) {
            if showsGlobe {
                key(systemImage: "globe", width: 44, action: advance)
            }
            key(text: showsLetters ? "Pennel" : Copy.Keyboard.abc, width: 60) { showsLetters.toggle() }
            if showsLetters { key(text: ",", width: 36) { insert(",") } }
            key(text: "spatie") { insert(" ") }
            if showsLetters { key(text: ".", width: 36) { insert(".") } }
            if !showsLetters { key(systemImage: "delete.left", width: 48, action: delete) }
            key(systemImage: "return", width: 48) { insert("\n") }
        }
    }

    private func key(text: String? = nil, systemImage: String? = nil, width: CGFloat? = nil, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Group {
                if let text {
                    Text(text).font(.figtree(Brand.FontSize.body, weight: .medium))
                } else if let systemImage {
                    Image(systemName: systemImage)
                }
            }
            .foregroundStyle(Brand.foreground.color)
            .frame(maxWidth: width ?? .infinity, minHeight: 42)
            .background(Brand.subtle.color, in: RoundedRectangle(cornerRadius: Brand.Radius.keycap))
        }
        .buttonStyle(.plain)
        .frame(width: width)
    }
}
