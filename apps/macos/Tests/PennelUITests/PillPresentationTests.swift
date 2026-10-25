import AppKit
import CoreText
import PennelKit
@testable import Pennel
import SwiftUI
import XCTest

final class PillPresentationTests: XCTestCase {
    @MainActor
    func testWorkingKeepsTheCaptionAndHiddenClearsItsOwner() async {
        let model = PillModel()
        model.show(.listening)
        model.preview.begin("one")
        model.preview.update(sessionID: "one", revision: 1, text: "Voorlopige woorden")
        model.show(.working)
        XCTAssertEqual(model.preview.text, "Voorlopige woorden")
        model.show(.hidden)
        XCTAssertNil(model.preview.sessionID)
        XCTAssertEqual(model.preview.text, "")
    }

    @MainActor
    func testPreviewGrowsTheCompactPillWithinSharedWindowLimits() async {
        registerFonts()
        let model = PillModel()
        model.show(.listening)
        let host = NSHostingView(rootView: PillView(model: model))
        host.setFrameSize(NSSize(width: ThemePill.windowWidth, height: ThemePill.windowHeight))
        host.layoutSubtreeIfNeeded()
        let compact = host.fittingSize
        model.preview.begin("one")
        model.preview.update(sessionID: "one", revision: 1, text: "Zullen we morgen om tien uur afspreken? Dan kunnen we het plan samen doornemen.")
        await Task.yield()
        host.layoutSubtreeIfNeeded()
        let caption = host.fittingSize
        XCTAssertGreaterThan(caption.width, compact.width)
        XCTAssertGreaterThan(caption.height, compact.height)
        XCTAssertLessThanOrEqual(caption.width, ThemePill.windowWidth)
        XCTAssertLessThanOrEqual(caption.height, ThemePill.windowHeight)
    }

    @MainActor
    func testLongCaptionDoesNotDrawOutsideItsLatestTwoLineViewport() async throws {
        registerFonts()
        let width = ThemePill.windowWidth - 2 * (ThemePill.outerInset + ThemeSpacing.md)
        let band = ThemePill.previewLineHeight
        let viewport = band * CGFloat(ThemePill.previewLines)
        let imageHeight = Int(viewport + 2 * band)
        let bitmap = try XCTUnwrap(NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(width), pixelsHigh: imageHeight,
            bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0))
        let context = try XCTUnwrap(NSGraphicsContext(bitmapImageRep: bitmap))
        let view = CaptionTailView(frame: NSRect(x: 0, y: 0, width: width, height: viewport))
        view.text = String(repeating: "Deze oudere woorden horen niet buiten de pil te verschijnen. ", count: 4) + "De laatste woorden blijven zichtbaar."
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = context
        context.cgContext.translateBy(x: 0, y: band)
        view.draw(view.bounds)
        context.flushGraphics()
        NSGraphicsContext.restoreGraphicsState()
        var inside = 0
        var outside = 0
        for y in 0..<imageHeight {
            for x in 0..<Int(width) {
                guard let color = bitmap.colorAt(x: x, y: y)?.usingColorSpace(.deviceRGB), color.alphaComponent > 0.7,
                      color.redComponent > 0.7, color.greenComponent > 0.7, color.blueComponent > 0.7 else { continue }
                if y < Int(band) || y >= Int(band + viewport) { outside += 1 }
                else { inside += 1 }
            }
        }
        XCTAssertGreaterThan(inside, 20)
        XCTAssertEqual(outside, 0)
    }

    private func registerFonts() {
        var root = URL(fileURLWithPath: #filePath)
        for _ in 0..<5 { root.deleteLastPathComponent() }
        let fonts = root.appendingPathComponent("assets/brand/fonts")
        for name in ["Figtree-Regular.ttf", "Figtree-Medium.ttf", "Figtree-SemiBold.ttf"] {
            CTFontManagerRegisterFontsForURL(fonts.appendingPathComponent(name) as CFURL, .process, nil)
        }
    }
}
