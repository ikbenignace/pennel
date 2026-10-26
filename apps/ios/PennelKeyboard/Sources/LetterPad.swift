import PennelMobileKit
import SwiftUI

struct LetterPad: View {
    let layout: KeyboardLayout
    let insert: (String) -> Void
    let delete: () -> Void
    @State private var shifted = false

    var body: some View {
        VStack(spacing: Brand.Spacing.s2) {
            ForEach(Array(layout.rows.enumerated()), id: \.offset) { index, row in
                HStack(spacing: 5) {
                    if index == layout.rows.count - 1 {
                        key(systemImage: shifted ? "shift.fill" : "shift", width: 44) { shifted.toggle() }
                    }
                    ForEach(row, id: \.self) { letter in
                        let text = shifted ? letter.uppercased() : letter
                        key(text: text) {
                            insert(text)
                            shifted = false
                        }
                    }
                    if index == layout.rows.count - 1 {
                        key(systemImage: "delete.left", width: 44, action: delete)
                    }
                }
                .padding(.horizontal, index == 1 ? Brand.Spacing.s3 : 0)
            }
        }
    }

    private func key(text: String? = nil, systemImage: String? = nil, width: CGFloat? = nil, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Group {
                if let text {
                    Text(text).font(.figtree(Brand.FontSize.title, weight: .medium))
                } else if let systemImage {
                    Image(systemName: systemImage)
                }
            }
            .foregroundStyle(Brand.foreground.color)
            .frame(maxWidth: width ?? .infinity, minHeight: 42)
            .background(Brand.field.color, in: RoundedRectangle(cornerRadius: Brand.Radius.keycap))
            .shadow(color: Brand.keyShadow.color, radius: 0, y: 1)
        }
        .buttonStyle(.plain)
        .frame(width: width)
    }
}
