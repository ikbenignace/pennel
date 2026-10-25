import Testing
import UIKit
@testable import Pennel

struct SettingsTests {
    @Test func settings_sections_match_desktop_order() {
        #expect(SettingsSection.allCases.map(\.title) == [
            "Providers", "Modellen", "Dicteren", "Meetings", "Opnemen", "Woordenlijst", "Stijlen", "Weergave", "Account",
        ])
    }

    @Test func brand_fonts_are_registered() {
        for name in ["Figtree-Regular", "Figtree-Medium", "Figtree-SemiBold", "InstrumentSerif-Regular"] {
            #expect(UIFont(name: name, size: 17) != nil, "\(name)")
        }
    }
}
