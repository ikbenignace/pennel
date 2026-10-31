import Testing
import UIKit
@testable internal import PennelMobile

@MainActor
struct SettingsTests {
    @Test func brand_fonts_are_registered() {
        for name in ["Figtree-Regular", "Figtree-Medium", "Figtree-SemiBold", "InstrumentSerif-Regular"] {
            #expect(UIFont(name: name, size: 17) != nil, "\(name)")
        }
    }
}
