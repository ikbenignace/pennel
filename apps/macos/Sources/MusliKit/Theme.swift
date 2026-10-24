import AppKit
import SwiftUI

public enum Theme {
    public static let paper = Color(hex: 0xFCFBF8)
    public static let background = Color(hex: 0xF7F4EE)
    public static let ink = Color(hex: 0x2A2723)
    public static let muted = Color(hex: 0x9B948A)
    public static let line = Color(hex: 0xECE7DE)
    public static let accent = Color(hex: 0xC2410C)
    public static let wave = Color(hex: 0xF2B48E)
    private static let voices = [Color(hex: 0x2F6F62), Color(hex: 0x6D4C8F), Color(hex: 0x2F5D8A), Color(hex: 0x8A6A2F)]

    public static func voice(_ number: UInt32) -> Color {
        voices[Int(number - 1) % voices.count]
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
        .custom("Figtree", size: size).weight(weight)
    }
}
