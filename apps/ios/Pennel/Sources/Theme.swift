import PennelMobileKit
import SwiftUI
import UIKit

/// iOS text sizes follow Dynamic Type; only the families and colours come from the brand.
enum Theme {
    static func font(_ style: UIFont.TextStyle, weight: Font.Weight = .regular) -> Font {
        .figtree(UIFont.preferredFont(forTextStyle: style).pointSize, weight: weight)
    }

    @MainActor
    static func configureNavigationBars() {
        let largeTitle = UIFont(name: "InstrumentSerif-Regular", size: UIFont.preferredFont(forTextStyle: .largeTitle).pointSize)
        let title = UIFont(name: "Figtree-SemiBold", size: UIFont.preferredFont(forTextStyle: .headline).pointSize)
        let bar = UINavigationBar.appearance()
        if let largeTitle {
            bar.largeTitleTextAttributes = [.font: UIFontMetrics(forTextStyle: .largeTitle).scaledFont(for: largeTitle)]
        }
        if let title {
            bar.titleTextAttributes = [.font: UIFontMetrics(forTextStyle: .headline).scaledFont(for: title)]
        }
    }
}

extension View {
    /// One calm background behind native lists, rows a shade apart instead of bordered panels.
    func pennelList() -> some View {
        scrollContentBackground(.hidden)
            .background(Brand.surface.color)
    }

    func pennelRow() -> some View {
        listRowBackground(Brand.subtle.color)
    }
}

extension AppearancePreference {
    var colorScheme: ColorScheme? {
        switch self {
        case .system: nil
        case .light: .light
        case .dark: .dark
        }
    }
}
