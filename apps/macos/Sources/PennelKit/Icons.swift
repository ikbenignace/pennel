import AppKit
import SwiftUI

public extension PennelIcon {
    var nsImage: NSImage {
        let bundled = Bundle.main.url(forResource: rawValue, withExtension: "svg", subdirectory: "Icons")
        let image = bundled.flatMap { NSImage(contentsOf: $0) }
            ?? NSImage(data: Data(svgSource.utf8))
            ?? NSImage(size: NSSize(width: 24, height: 24))
        image.isTemplate = true
        return image
    }

    var image: Image {
        Image(nsImage: nsImage)
    }
}
