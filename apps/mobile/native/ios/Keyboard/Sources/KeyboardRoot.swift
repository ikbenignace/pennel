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
    let feedback: (FeedbackEvent) -> Void
    let heightChanged: (CGFloat) -> Void
    @State private var showsLetters = true
    @State private var showsHelp = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @ScaledMetric(relativeTo: .body) private var scaledKeyHeight = Brand.Spacing.s10 + Brand.Spacing.s1
    private let controlWidth = Brand.Spacing.s10 + Brand.Spacing.s1
    private var keyHeight: CGFloat { max(controlWidth, min(scaledKeyHeight, Brand.Spacing.s10 + Brand.Spacing.s8)) }

    var body: some View {
        VStack(spacing: Brand.Spacing.s1) {
            if showsLetters { toolbar }
            if showsLetters {
                if showsHelp { guidance }
                if let preview = model.lastResultPreview { recovery(preview) }
                if typing.suggestionsEnabled { suggestionRow }
                LetterPad(layout: layout, typing: typing, insert: insert, delete: delete, feedback: feedback)
            } else {
                DictationPad(model: model, openSettings: openSettings)
            }
            bottomRow
        }
        .padding(.horizontal, Brand.Spacing.s2)
        .padding(.vertical, Brand.Spacing.s2)
        .background(Brand.surface.color)
        .fixedSize(horizontal: false, vertical: true)
        .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { heightChanged($0) }
        .animation(reduceMotion ? nil : .easeOut(duration: 0.15), value: showsHelp)
        .onChange(of: model.pad) { _, pad in
            if pad == .ready || pad == .listening || pad == .processing { showsHelp = false }
            if case .failed = pad { showsHelp = true }
        }
    }

    private var toolbar: some View {
        HStack(spacing: Brand.Spacing.s2) {
            Button { showsLetters = false; feedback(.selection) } label: {
                Text("Pennel").font(.figtree(Brand.FontSize.content, weight: .semibold))
                    .foregroundStyle(Brand.foreground.color)
                    .frame(minWidth: controlWidth, minHeight: keyHeight)
                    .contentShape(Rectangle())
            }
            .accessibilityLabel(typing.copy.text(.showDictation))
            HStack(spacing: Brand.Spacing.s2) {
                Image(systemName: model.pad == .listening ? "stop.fill" : model.pad == .processing ? "ellipsis" : "mic.fill")
                Text(status).lineLimit(1)
            }
            .font(.figtree(Brand.FontSize.body, weight: .medium))
            .foregroundStyle(Brand.pillText.color)
            .frame(maxWidth: .infinity, minHeight: keyHeight)
            .padding(.horizontal, Brand.Spacing.s3)
            .background(Brand.pillBackground.color, in: Capsule())
            .accessibilityHidden(true)
            .overlay {
                KeyboardDictationGesture(label: model.primaryActionLabel,
                    hint: model.pad == .sessionOff ? typing.copy.text(.manualStart) : "",
                    value: actionValue, enabled: model.pad != .processing, holdEnabled: model.pad == .ready,
                    tap: microphoneTapped, holdBegan: model.holdBegan, holdEnded: model.holdEnded)
            }
            if model.pad == .listening || model.pad == .processing {
                Button { model.cancel() } label: {
                    Image(systemName: "xmark.circle.fill").frame(width: controlWidth, height: keyHeight).contentShape(Rectangle())
                }
                .accessibilityLabel(typing.copy.text(.cancel))
            }
            Button(action: toggleSuggestions) {
                Image(systemName: typing.suggestionsEnabled ? "textformat.abc" : "textformat.abc.dottedunderline")
                    .foregroundStyle(typing.suggestionsEnabled ? Brand.accent.color : Brand.muted.color)
                    .frame(width: controlWidth, height: keyHeight)
                    .contentShape(Rectangle())
            }
            .accessibilityLabel(typing.copy.text(typing.suggestionsEnabled ? .suggestions : .suggestionsOff))
            .accessibilityHint(typing.copy.text(.suggestionsPrivacy))
        }
        .frame(minHeight: keyHeight)
    }

    private var guidance: some View {
        HStack(alignment: .top, spacing: Brand.Spacing.s2) {
            VStack(alignment: .leading, spacing: Brand.Spacing.s1) {
                switch model.pad {
                case .needsFullAccess:
                    Text(typing.copy.text(.fullAccess))
                    Button(action: openSettings) {
                        Text(typing.copy.text(.settings)).frame(maxWidth: .infinity, minHeight: controlWidth, alignment: .leading).contentShape(Rectangle())
                    }
                case .failed(let message):
                    Text(message)
                    Button {
                        if model.recoveryAction == .recordAgain { model.recordAgain() } else { model.retry() }
                    } label: {
                        Text(model.primaryActionLabel).frame(maxWidth: .infinity, minHeight: controlWidth, alignment: .leading).contentShape(Rectangle())
                    }
                default:
                    Text(typing.copy.text(.manualStart))
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            Button { showsHelp = false } label: {
                Image(systemName: "xmark").frame(width: controlWidth, height: controlWidth).contentShape(Rectangle())
            }
            .accessibilityLabel(typing.copy.text(.hideHelp))
        }
        .font(.figtree(Brand.FontSize.content))
        .foregroundStyle(Brand.foreground.color)
        .padding(.horizontal, Brand.Spacing.s2)
    }

    private func recovery(_ preview: String) -> some View {
        VStack(alignment: .leading, spacing: Brand.Spacing.s1) {
            Text(preview)
                .font(.figtree(Brand.FontSize.content))
                .foregroundStyle(Brand.muted.color)
                .lineLimit(2)
            Button { model.insertLast() } label: {
                Text(typing.copy.text(.insertLast))
                    .font(.figtree(Brand.FontSize.content, weight: .semibold))
                    .foregroundStyle(Brand.accent.color)
                    .frame(maxWidth: .infinity, minHeight: controlWidth)
                    .contentShape(Rectangle())
            }
        }
        .padding(.horizontal, Brand.Spacing.s2)
    }

    private func microphoneTapped() {
        switch model.pad {
        case .sessionOff, .needsFullAccess:
            showsHelp.toggle()
            feedback(.selection)
        default: model.tapPill()
        }
    }

    private var actionValue: String {
        switch model.pad {
        case .listening: typing.copy.text(.listening)
        case .sessionOff: typing.copy.text(.sessionOff)
        case .needsFullAccess: typing.copy.text(.fullAccess)
        default: ""
        }
    }

    private var status: String {
        switch model.pad {
        case .ready: typing.copy.text(.ready)
        case .listening: typing.copy.text(.listening)
        case .processing: typing.copy.text(.processing)
        case .needsFullAccess: typing.copy.text(.setupDictation)
        case .sessionOff: typing.copy.text(.sessionOff)
        case .failed: model.primaryActionLabel
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
                    Button { insertSuggestion(suggestion) } label: {
                        Text(suggestion)
                            .foregroundStyle(Brand.foreground.color)
                            .frame(maxWidth: .infinity, minHeight: controlWidth)
                            .lineLimit(2)
                            .contentShape(Rectangle())
                    }
                    .accessibilityHint(typing.copy.text(.insertSuggestion))
                }
            }
        }
        .font(.figtree(Brand.FontSize.content, weight: .medium))
        .frame(minHeight: controlWidth)
    }

    private var bottomRow: some View {
        HStack(spacing: 5) {
            if showsGlobe {
                KeyboardGlobeKey(target: inputModeTarget, label: typing.copy.text(.nextKeyboard))
                    .frame(width: controlWidth, height: keyHeight)
            }
            key(text: !showsLetters || typing.state.layer != .letters ? typing.copy.text(.letters) : typing.copy.text(.numbers),
                width: controlWidth + Brand.Spacing.s3, label: typing.copy.text(!showsLetters ? .showTyping : typing.state.layer != .letters ? .showLetters : .showNumbers)) {
                if !showsLetters { showsLetters = true } else { typing.state.layer = typing.state.layer == .letters ? .numbers : .letters }
                feedback(.keyPress)
            }
            if showsLetters { key(text: typing.showsAt ? "@" : ",", width: controlWidth - Brand.Spacing.s2) { insert(typing.showsAt ? "@" : ",") } }
            key(text: typing.copy.text(.space), action: space)
            if showsLetters { key(text: typing.showsURL ? ".com" : ".", width: typing.showsURL ? controlWidth + Brand.Spacing.s2 : controlWidth - Brand.Spacing.s2) { insert(typing.showsURL ? ".com" : ".") } }
            if !showsLetters {
                KeyboardDeleteKey(label: typing.copy.text(.delete), action: delete, feedback: { feedback(.keyPress) }).frame(width: controlWidth, height: keyHeight)
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
