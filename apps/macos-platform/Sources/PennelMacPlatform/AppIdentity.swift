import AppKit
import Foundation

struct AppIdentity: Equatable {
    let name: String
    let iconDataURL: String
}

func appIdentity(bundleID: String, points: Int) -> AppIdentity? {
    guard !bundleID.isEmpty, bundleID.utf8.count <= 255,
          let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundleID) else { return nil }
    let bundle = Bundle(url: url)
    let name = (bundle?.localizedInfoDictionary?["CFBundleDisplayName"] as? String)
        ?? (bundle?.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String)
        ?? (bundle?.object(forInfoDictionaryKey: "CFBundleName") as? String)
        ?? url.deletingPathExtension().lastPathComponent
    guard let png = pngIcon(NSWorkspace.shared.icon(forFile: url.path), points: points) else { return nil }
    return AppIdentity(name: name, iconDataURL: "data:image/png;base64," + png.base64EncodedString())
}

/// Rendered at twice the point size so the icon stays sharp on Retina displays.
private func pngIcon(_ image: NSImage, points: Int) -> Data? {
    let pixels = max(16, min(points, 128)) * 2
    guard let bitmap = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels, bitsPerSample: 8, samplesPerPixel: 4,
        hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0
    ) else { return nil }
    NSGraphicsContext.saveGraphicsState()
    defer { NSGraphicsContext.restoreGraphicsState() }
    guard let context = NSGraphicsContext(bitmapImageRep: bitmap) else { return nil }
    NSGraphicsContext.current = context
    context.imageInterpolation = .high
    image.draw(in: NSRect(x: 0, y: 0, width: pixels, height: pixels), from: .zero, operation: .copy, fraction: 1)
    context.flushGraphics()
    return bitmap.representation(using: .png, properties: [:])
}

@_cdecl("pennel_mac_app_identity")
public func appIdentityJSON(_ rawID: UnsafePointer<CChar>?, _ points: Int32) -> UnsafeMutablePointer<CChar>? {
    guard let id = decodeString(rawID), let identity = onMain({ appIdentity(bundleID: id, points: Int(points)) }) else {
        return bridgeString("null")
    }
    return bridgeJSON(["name": identity.name, "icon": identity.iconDataURL])
}
