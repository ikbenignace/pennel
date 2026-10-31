import PennelMobileKit
import SwiftUI

struct LetterPad: View {
    let layout: KeyboardLayout
    let typing: KeyboardTypingModel
    let insert: (String) -> Void
    let delete: () -> Void
    @ScaledMetric(relativeTo: .body) private var scaledKeyHeight = Brand.Spacing.s10 + Brand.Spacing.s1
    private let controlWidth = Brand.Spacing.s10 + Brand.Spacing.s1
    private var keyHeight: CGFloat { max(controlWidth, min(scaledKeyHeight, Brand.Spacing.s10 + Brand.Spacing.s8)) }

    var body: some View {
        VStack(spacing: Brand.Spacing.s1) {
            ForEach(Array(typing.state.rows(layout: layout).enumerated()), id: \.offset) { index, row in
                HStack(spacing: 5) {
                    if index == 2 {
                        if typing.state.layer == .letters {
                            key(systemImage: shiftSymbol, width: controlWidth, label: typing.copy.text(typing.state.shift == .locked ? .capsLock : .shift)) {
                                typing.state.pressShift(at: Date())
                            }
                        } else {
                            key(text: typing.copy.text(typing.state.layer == .numbers ? .symbols : .numbers), width: controlWidth + Brand.Spacing.s2,
                                label: typing.copy.text(typing.state.layer == .numbers ? .showSymbols : .showNumbers)) {
                                typing.state.layer = typing.state.layer == .numbers ? .symbols : .numbers
                            }
                        }
                    }
                    ForEach(row, id: \.self) { letter in
                        let text = typing.state.text(for: letter)
                        key(text: text) {
                            insert(text)
                        }
                    }
                    if index == 2 {
                        KeyboardDeleteKey(label: typing.copy.text(.delete), action: delete)
                            .frame(width: controlWidth, height: keyHeight)
                    }
                }
                .padding(.horizontal, index == 1 ? Brand.Spacing.s3 : 0)
            }
        }
    }

    private var shiftSymbol: String {
        switch typing.state.shift {
        case .off: "shift"
        case .once: "shift.fill"
        case .locked: "capslock.fill"
        }
    }

    private func key(text: String? = nil, systemImage: String? = nil, width: CGFloat? = nil, label: String? = nil, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Group {
                if let text {
                    Text(text).font(.figtree(Brand.FontSize.title, weight: .medium)).lineLimit(1).minimumScaleFactor(0.7)
                } else if let systemImage {
                    Image(systemName: systemImage)
                }
            }
            .foregroundStyle(Brand.foreground.color)
            .frame(maxWidth: .infinity, minHeight: keyHeight)
            .background(Brand.field.color, in: RoundedRectangle(cornerRadius: Brand.Radius.keycap))
            .shadow(color: Brand.keyShadow.color, radius: 0, y: 1)
        }
        .buttonStyle(.plain)
        .frame(width: width)
        .accessibilityLabel(label ?? text ?? "")
    }
}
