import PennelMobileKit
import SwiftUI

struct KeyboardRoot: View {
    let model: KeyboardModel
    let typing: KeyboardTypingModel
    let layout: KeyboardLayout
    let showsGlobe: Bool
    let inputModeTarget: KeyboardInputModeTarget
    let openSettings: () -> Void
    let insert: (String) -> Void
    let delete: () -> Void
    let space: () -> Void
    let insertSuggestion: (String) -> Void
    let toggleSuggestions: () -> Void
    @State private var showsLetters = true
    @ScaledMetric(relativeTo: .body) private var scaledKeyHeight = Brand.Spacing.s10 + Brand.Spacing.s1
    private let controlWidth = Brand.Spacing.s10 + Brand.Spacing.s1
    private var keyHeight: CGFloat { max(controlWidth, min(scaledKeyHeight, Brand.Spacing.s10 + Brand.Spacing.s8)) }

    var body: some View {
        VStack(spacing: Brand.Spacing.s1) {
            if showsLetters { toolbar }
            if showsLetters {
                if typing.suggestionsEnabled { suggestionRow }
                LetterPad(layout: layout, typing: typing, insert: insert, delete: delete)
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

    private var toolbar: some View {
        HStack(spacing: Brand.Spacing.s2) {
            Button { showsLetters = false } label: {
                Text("Pennel").font(.figtree(Brand.FontSize.content, weight: .semibold))
                    .foregroundStyle(Brand.foreground.color)
            }
            Button { microphoneTapped() } label: {
                HStack(spacing: Brand.Spacing.s2) {
                    Image(systemName: model.pad == .listening ? "stop.fill" : "mic.fill")
                    Text(status).lineLimit(1)
                }
                .font(.figtree(Brand.FontSize.body, weight: .medium))
                .foregroundStyle(Brand.pillText.color)
                .frame(maxWidth: .infinity, minHeight: keyHeight)
                .padding(.horizontal, Brand.Spacing.s3)
                .background(Brand.pillBackground.color, in: Capsule())
            }
            .buttonStyle(.plain)
            .onLongPressGesture(minimumDuration: 0.3, maximumDistance: 60) {
                if model.pad == .sessionOff || model.pad == .needsFullAccess { showsLetters = false }
                model.holdBegan()
            } onPressingChanged: { pressing in
                if !pressing { model.holdEnded() }
            }
            .accessibilityLabel(typing.copy.text(.microphone) + ", " + status)
            .accessibilityHint(model.pad == .sessionOff ? typing.copy.text(.manualStart) : "")
            if model.pad == .listening || model.pad == .processing {
                Button { model.cancel() } label: { Image(systemName: "xmark.circle.fill") }
                    .accessibilityLabel(typing.copy.text(.cancel))
            }
            Button(action: toggleSuggestions) {
                Image(systemName: typing.suggestionsEnabled ? "textformat.abc" : "textformat.abc.dottedunderline")
                    .foregroundStyle(typing.suggestionsEnabled ? Brand.accent.color : Brand.muted.color)
            }
            .accessibilityLabel(typing.copy.text(typing.suggestionsEnabled ? .suggestions : .suggestionsOff))
            .accessibilityHint(typing.copy.text(.suggestionsPrivacy))
        }
        .frame(minHeight: keyHeight)
        .overlay(alignment: .bottom) {
            if model.lastResult != nil {
                Button(typing.copy.text(.insertLast)) { model.insertLast() }
                    .font(.figtree(Brand.FontSize.label, weight: .semibold))
                    .foregroundStyle(Brand.accent.color)
                    .offset(y: Brand.Spacing.s4)
            }
        }
        .padding(.bottom, model.lastResult == nil ? 0 : Brand.Spacing.s5)
    }

    private func microphoneTapped() {
        if model.pad == .sessionOff || model.pad == .needsFullAccess { showsLetters = false }
        if case .failed = model.pad { showsLetters = false }
        model.tapPill()
    }

    private var status: String {
        switch model.pad {
        case .ready: typing.copy.text(.ready)
        case .listening: typing.copy.text(.listening)
        case .processing: typing.copy.text(.processing)
        case .needsFullAccess: typing.copy.text(.fullAccess)
        case .sessionOff: typing.copy.text(.sessionOff)
        case .failed: typing.copy.text(.retry)
        }
    }

    private var suggestionRow: some View {
        HStack(spacing: Brand.Spacing.s2) {
            if typing.suggestions.isEmpty {
                Text(typing.copy.text(.suggestionsPrivacy))
                    .foregroundStyle(Brand.muted.color)
                    .frame(maxWidth: .infinity)
            } else {
                ForEach(typing.suggestions, id: \.self) { suggestion in
                    Button(suggestion) { insertSuggestion(suggestion) }
                        .foregroundStyle(Brand.foreground.color)
                        .frame(maxWidth: .infinity)
                        .lineLimit(1)
                        .accessibilityHint(typing.copy.text(.insertSuggestion))
                }
            }
        }
        .font(.figtree(Brand.FontSize.content, weight: .medium))
        .frame(height: Brand.Spacing.s8)
    }

    private var bottomRow: some View {
        HStack(spacing: 5) {
            if showsGlobe {
                KeyboardGlobeKey(target: inputModeTarget, label: typing.copy.text(.nextKeyboard))
                    .frame(width: controlWidth, height: keyHeight)
            }
            key(text: !showsLetters || typing.state.layer != .letters ? typing.copy.text(.letters) : typing.copy.text(.numbers),
                width: controlWidth + Brand.Spacing.s3, label: typing.copy.text(!showsLetters || typing.state.layer != .letters ? .showLetters : .showNumbers)) {
                if !showsLetters { showsLetters = true } else { typing.state.layer = typing.state.layer == .letters ? .numbers : .letters }
            }
            if showsLetters { key(text: typing.showsAt ? "@" : ",", width: controlWidth - Brand.Spacing.s2) { insert(typing.showsAt ? "@" : ",") } }
            key(text: typing.copy.text(.space), action: space)
            if showsLetters { key(text: typing.showsURL ? ".com" : ".", width: typing.showsURL ? controlWidth + Brand.Spacing.s2 : controlWidth - Brand.Spacing.s2) { insert(typing.showsURL ? ".com" : ".") } }
            if !showsLetters {
                KeyboardDeleteKey(label: typing.copy.text(.delete), action: delete).frame(width: controlWidth, height: keyHeight)
            }
            key(text: typing.copy.text(typing.returnKey), width: controlWidth + Brand.Spacing.s5, enabled: typing.returnEnabled) { insert("\n") }
        }
    }

    private func key(text: String? = nil, systemImage: String? = nil, width: CGFloat? = nil, label: String? = nil, enabled: Bool = true, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Group {
                if let text {
                    Text(text).font(.figtree(Brand.FontSize.body, weight: .medium)).lineLimit(1).minimumScaleFactor(0.7)
                } else if let systemImage {
                    Image(systemName: systemImage)
                }
            }
            .foregroundStyle(enabled ? Brand.foreground.color : Brand.disabledText.color)
            .frame(maxWidth: .infinity, minHeight: keyHeight)
            .background(Brand.subtle.color, in: RoundedRectangle(cornerRadius: Brand.Radius.keycap))
        }
        .buttonStyle(.plain)
        .frame(width: width)
        .accessibilityLabel(label ?? text ?? "")
        .disabled(!enabled)
    }
}
