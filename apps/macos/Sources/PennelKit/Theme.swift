import AppKit
import SwiftUI

public enum Theme {
    public static let surface = dynamic(ThemeTokens.surface, named: "surface")
    public static let chrome = dynamic(ThemeTokens.chrome, named: "chrome")
    public static let sidebar = dynamic(ThemeTokens.sidebar, named: "sidebar")
    public static let foreground = dynamic(ThemeTokens.foreground, named: "foreground")
    public static let muted = dynamic(ThemeTokens.muted, named: "muted")
    public static let border = dynamic(ThemeTokens.border, named: "border")
    public static let subtle = dynamic(ThemeTokens.subtle, named: "subtle")
    public static let hover = dynamic(ThemeTokens.hover, named: "hover")
    public static let field = dynamic(ThemeTokens.field, named: "field")
    public static let selection = dynamic(ThemeTokens.selection, named: "selection")
    public static let button = dynamic(ThemeTokens.button, named: "button")
    public static let buttonText = dynamic(ThemeTokens.buttonText, named: "buttonText")
    public static let accent = dynamic(ThemeTokens.accent, named: "accent")
    public static let focusRing = dynamic(ThemeTokens.focusRing, named: "focusRing")
    public static let disabled = dynamic(ThemeTokens.disabled, named: "disabled")
    public static let disabledText = dynamic(ThemeTokens.disabledText, named: "disabledText")
    public static let shadow = dynamic(ThemeTokens.shadow, named: "shadow")
    public static let keyShadow = dynamic(ThemeTokens.keyShadow, named: "keyShadow")

    public static let paper = surface
    public static let background = subtle
    public static let ink = foreground
    public static let line = border
    public static let action = button
    public static let actionForeground = buttonText
    public static let control = field
    public static let controlForeground = foreground
    public static let error = accent
    public static let wave = dynamic(ThemeTokens.wave, named: "wave")
    public static let pillBackground = dynamic(ThemeTokens.pillBackground, named: "pillBackground")
    public static let pillText = dynamic(ThemeTokens.pillText, named: "pillText")
    public static let pillForeground = pillText
    public static let pillAccent = dynamic(ThemeTokens.pillAccent, named: "pillAccent")

    private static let voices = [
        dynamic(ThemeTokens.voice1, named: "voice1"),
        dynamic(ThemeTokens.voice2, named: "voice2"),
        dynamic(ThemeTokens.voice3, named: "voice3"),
        dynamic(ThemeTokens.voice4, named: "voice4"),
    ]

    public static func voice(_ number: UInt32) -> Color {
        voices[Int(number &- 1) % voices.count]
    }

    private static func dynamic(_ pair: ThemeTokens.ColorPair, named name: String) -> Color {
        Color(nsColor: NSColor(name: NSColor.Name("Pennel.\(name)")) { appearance in
            let dark = appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
            let value = dark ? pair.dark : pair.light
            let red = CGFloat((value >> 16) & 0xFF) / 255
            let green = CGFloat((value >> 8) & 0xFF) / 255
            let blue = CGFloat(value & 0xFF) / 255
            let alpha = CGFloat(dark ? pair.darkAlpha : pair.lightAlpha)
            return NSColor(srgbRed: red, green: green, blue: blue, alpha: alpha)
        })
    }
}

public extension Color {
    init(hex: UInt32) {
        self.init(
            red: Double((hex >> 16) & 0xFF) / 255,
            green: Double((hex >> 8) & 0xFF) / 255,
            blue: Double(hex & 0xFF) / 255
        )
    }
}

extension Font {
    /// Figtree is bundled via ATSApplicationFontsPath; SwiftUI falls back to the system font when it is missing.
    public static func figtree(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        .custom(ThemeTypography.ui, size: size).weight(weight)
    }
}
