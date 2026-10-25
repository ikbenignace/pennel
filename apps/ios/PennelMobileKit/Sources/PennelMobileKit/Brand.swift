import SwiftUI
#if canImport(UIKit)
import UIKit
#elseif canImport(AppKit)
import AppKit
#endif

/// Values mirror `assets/brand/tokens.json`, the owner of the palette; `BrandTests` parses that file and fails on drift.
public enum Brand {
    public struct Token: Equatable, Sendable {
        public let light: UInt32
        public let dark: UInt32
        public let lightAlpha: Double
        public let darkAlpha: Double

        public init(light: UInt32, dark: UInt32, lightAlpha: Double = 1, darkAlpha: Double = 1) {
            self.light = light
            self.dark = dark
            self.lightAlpha = lightAlpha
            self.darkAlpha = darkAlpha
        }

        public var color: Color {
            #if canImport(UIKit)
            Color(uiColor: UIColor {
                $0.userInterfaceStyle == .dark ? UIColor(hex: dark, alpha: darkAlpha) : UIColor(hex: light, alpha: lightAlpha)
            })
            #else
            Color(nsColor: NSColor(name: nil) { appearance in
                appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
                    ? NSColor(hex: dark, alpha: darkAlpha) : NSColor(hex: light, alpha: lightAlpha)
            })
            #endif
        }
    }

    public static let surface = Token(light: 0xFCFBF8, dark: 0x1E1C1A)
    public static let chrome = Token(light: 0xFCFBF8, dark: 0x1E1C1A)
    public static let sidebar = Token(light: 0xF7F4EE, dark: 0x252320)
    public static let foreground = Token(light: 0x2A2723, dark: 0xF3F0EA)
    public static let muted = Token(light: 0x6F675D, dark: 0xABA398)
    public static let border = Token(light: 0xECE7DE, dark: 0x3A3632)
    public static let subtle = Token(light: 0xF7F4EE, dark: 0x282522)
    public static let hover = Token(light: 0xF1EDE6, dark: 0x312E2A)
    public static let field = Token(light: 0xFFFFFF, dark: 0x2C2926)
    public static let selection = Token(light: 0xECE7DE, dark: 0x3A3632)
    public static let button = Token(light: 0x2A2723, dark: 0xF3F0EA)
    public static let buttonText = Token(light: 0xFCFBF8, dark: 0x1E1C1A)
    public static let accent = Token(light: 0xC2410C, dark: 0xF2B48E)
    public static let focusRing = Token(light: 0xC2410C, dark: 0xF2B48E)
    public static let ready = Token(light: 0x34C759, dark: 0x30D158, lightAlpha: 0.70196078, darkAlpha: 0.70196078)
    public static let controlEdge = Token(light: 0x2A2723, dark: 0xFFFFFF, lightAlpha: 0.18039216, darkAlpha: 0.12156863)
    public static let disabled = Token(light: 0xF1EDE6, dark: 0x2C2926)
    public static let disabledText = Token(light: 0x6F675D, dark: 0xABA398)
    public static let shadow = Token(light: 0x2A2723, dark: 0x000000, lightAlpha: 0.10196078, darkAlpha: 0.33333333)
    public static let keyShadow = Token(light: 0xE2DCD1, dark: 0x0F0E0D)
    public static let pillBackground = Token(light: 0x2A2723, dark: 0x2A2723)
    public static let pillText = Token(light: 0xFCFBF8, dark: 0xFCFBF8)
    public static let pillAccent = Token(light: 0xF2B48E, dark: 0xF2B48E)
    public static let brandPaper = Token(light: 0xFCFBF8, dark: 0xFCFBF8)
    public static let brandInk = Token(light: 0x2A2723, dark: 0x2A2723)
    public static let wave = Token(light: 0xF2B48E, dark: 0xF2B48E)
    public static let voice1 = Token(light: 0x2F6F62, dark: 0x80C5B1)
    public static let voice2 = Token(light: 0x6D4C8F, dark: 0xB9A0DB)
    public static let voice3 = Token(light: 0x2F5D8A, dark: 0x92BAE3)
    public static let voice4 = Token(light: 0x8A6A2F, dark: 0xD7BB83)

    /// Keyed by the colour names used in tokens.json.
    public static let colors: [String: Token] = [
        "surface": surface,
        "chrome": chrome,
        "sidebar": sidebar,
        "foreground": foreground,
        "muted": muted,
        "border": border,
        "subtle": subtle,
        "hover": hover,
        "field": field,
        "selection": selection,
        "button": button,
        "button-text": buttonText,
        "accent": accent,
        "focus-ring": focusRing,
        "ready": ready,
        "control-edge": controlEdge,
        "disabled": disabled,
        "disabled-text": disabledText,
        "shadow": shadow,
        "key-shadow": keyShadow,
        "pill-background": pillBackground,
        "pill-text": pillText,
        "pill-accent": pillAccent,
        "brand-paper": brandPaper,
        "brand-ink": brandInk,
        "wave": wave,
        "voice-1": voice1,
        "voice-2": voice2,
        "voice-3": voice3,
        "voice-4": voice4,
    ]

    public static let paper: UInt32 = 0xFCFBF8
    public static let ink: UInt32 = 0x2A2723
    public static let softWave: UInt32 = 0xF2B48E

    public enum Spacing {
        public static let s1: CGFloat = 4
        public static let s2: CGFloat = 8
        public static let s3: CGFloat = 12
        public static let s4: CGFloat = 16
        public static let s5: CGFloat = 20
        public static let s6: CGFloat = 24
        public static let s7: CGFloat = 28
        public static let s8: CGFloat = 32
        public static let s9: CGFloat = 36
        public static let s10: CGFloat = 40
    }

    public enum Radius {
        public static let control: CGFloat = 6
        public static let segmented: CGFloat = 8
        public static let panel: CGFloat = 12
        public static let window: CGFloat = 12
        public static let keycap: CGFloat = 11
        public static let pill: CGFloat = 9999
    }

    public enum FontSize {
        public static let caption: CGFloat = 11
        public static let label: CGFloat = 12
        public static let body: CGFloat = 13
        public static let content: CGFloat = 14
        public static let section: CGFloat = 16
        public static let title: CGFloat = 21
        public static let onboarding: CGFloat = 26
        public static let welcome: CGFloat = 28
        public static let wordmark: CGFloat = 29
        public static let pageTitle: CGFloat = 28
        public static let displayTitle: CGFloat = 30
    }

    public enum Wave {
        public static let barCount = 5
        public static let restingHeights: [CGFloat] = [0.35, 0.6, 0.85, 0.6, 0.35]
        public static let barWidthRatio: CGFloat = 0.12
        public static let spacingRatio: CGFloat = 0.09

        /// Bars grow from their resting height to the full height as `level` goes from 0 to 1.
        public static func barHeights(level: Float, height: CGFloat) -> [CGFloat] {
            let level = CGFloat(min(max(level, 0), 1))
            return restingHeights.map { height * ($0 + (1 - $0) * level) }
        }
    }
}

extension Color {
    public init(hex: UInt32) {
        self.init(
            red: Double((hex >> 16) & 0xFF) / 255,
            green: Double((hex >> 8) & 0xFF) / 255,
            blue: Double(hex & 0xFF) / 255)
    }
}

#if canImport(UIKit)
private extension UIColor {
    convenience init(hex: UInt32, alpha: Double = 1) {
        self.init(
            red: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255,
            blue: CGFloat(hex & 0xFF) / 255, alpha: CGFloat(alpha))
    }
}
#else
private extension NSColor {
    convenience init(hex: UInt32, alpha: Double = 1) {
        self.init(
            srgbRed: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255,
            blue: CGFloat(hex & 0xFF) / 255, alpha: CGFloat(alpha))
    }
}
#endif

extension Font {
    public static func figtree(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        let name = switch weight {
        case .medium: "Figtree-Medium"
        case .semibold, .bold, .heavy, .black: "Figtree-SemiBold"
        default: "Figtree-Regular"
        }
        return .custom(name, size: size)
    }

    public static func instrumentSerif(_ size: CGFloat) -> Font {
        .custom("InstrumentSerif-Regular", size: size)
    }
}
