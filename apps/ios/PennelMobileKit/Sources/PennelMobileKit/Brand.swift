import SwiftUI
#if canImport(UIKit)
import UIKit
#elseif canImport(AppKit)
import AppKit
#endif

/// Values mirror the palette table in `assets/brand/brand.md`; `BrandTests` parses that table and fails on drift.
public enum Brand {
    public struct Token: Equatable, Sendable {
        public let light: UInt32
        public let dark: UInt32

        public var color: Color {
            #if canImport(UIKit)
            Color(uiColor: UIColor { $0.userInterfaceStyle == .dark ? UIColor(hex: dark) : UIColor(hex: light) })
            #else
            Color(nsColor: NSColor(name: nil) { appearance in
                appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua ? NSColor(hex: dark) : NSColor(hex: light)
            })
            #endif
        }
    }

    public static let surface = Token(light: 0xFFFFFF, dark: 0x202022)
    public static let windowChrome = Token(light: 0xF6F6F8, dark: 0x27272A)
    public static let sidebar = Token(light: 0xF1F1F3, dark: 0x252527)
    public static let foreground = Token(light: 0x1D1D1F, dark: 0xF3F3F5)
    public static let secondaryText = Token(light: 0x64646A, dark: 0xA6A6AE)
    public static let border = Token(light: 0xE3E3E8, dark: 0x3B3B40)
    public static let selectedSurface = Token(light: 0xE3E3E8, dark: 0x3B3B40)
    public static let field = Token(light: 0xFFFFFF, dark: 0x2B2B2F)
    public static let primaryButton = Token(light: 0x252527, dark: 0xEAEAEC)
    public static let primaryButtonText = Token(light: 0xFFFFFF, dark: 0x202022)
    public static let accent = Token(light: 0xC2410C, dark: 0xF2B48E)

    /// Keyed by the role names used in brand.md.
    public static let roles: [String: Token] = [
        "Surface": surface,
        "Window chrome": windowChrome,
        "Sidebar": sidebar,
        "Foreground": foreground,
        "Secondary text": secondaryText,
        "Border": border,
        "Selected surface": selectedSurface,
        "Field": field,
        "Primary button": primaryButton,
        "Primary button text": primaryButtonText,
        "Accent": accent,
    ]

    public static let paper: UInt32 = 0xFCFBF8
    public static let ink: UInt32 = 0x2A2723
    public static let softWave: UInt32 = 0xF2B48E

    public static let pillBackground = ink
    public static let pillText = paper
    public static let pillIndicator = softWave

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
    convenience init(hex: UInt32) {
        self.init(
            red: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255,
            blue: CGFloat(hex & 0xFF) / 255, alpha: 1)
    }
}
#else
private extension NSColor {
    convenience init(hex: UInt32) {
        self.init(
            srgbRed: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255,
            blue: CGFloat(hex & 0xFF) / 255, alpha: 1)
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
