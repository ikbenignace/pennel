import AppKit
import Foundation

let root = URL(fileURLWithPath: FileManager.default.currentDirectoryPath, isDirectory: true)
let supplied = CommandLine.arguments.dropFirst().map { URL(fileURLWithPath: $0, isDirectory: true) }
let native = supplied.count > 0 ? supplied[0] : root.appendingPathComponent("build/visual-parity/native")
let desktop = supplied.count > 1 ? supplied[1] : root.appendingPathComponent("build/visual-parity/desktop")
let output = supplied.count > 2 ? supplied[2] : root.appendingPathComponent("build/visual-parity/comparison")
let nativeManifest = try JSONSerialization.jsonObject(with: Data(contentsOf: native.appendingPathComponent("fixture.json"))) as! [String: Any]
let desktopManifest = try JSONSerialization.jsonObject(with: Data(contentsOf: desktop.appendingPathComponent("manifest.json"))) as! [String: Any]
let nativeImages = nativeManifest["images"] as! [[String: Any]]
let desktopImages = desktopManifest["images"] as! [[String: Any]]
let byKey = Dictionary(uniqueKeysWithValues: desktopImages.map { (($0["appearance"] as! String) + "/" + ($0["case"] as! String), $0) })
var count = 0

for item in nativeImages {
    let appearance = item["appearance"] as! String
    let name = item["case"] as! String
    let key = appearance + "/" + name
    guard let other = byKey[key] else { continue }
    let width = item["logical_width"] as! Int
    let height = item["logical_height"] as! Int
    guard width == other["width"] as! Int, height == other["height"] as! Int else { continue }
    guard let left = NSImage(contentsOf: native.appendingPathComponent(item["file"] as! String)),
          let right = NSImage(contentsOf: desktop.appendingPathComponent(other["file"] as! String)),
          let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: width * 2, pixelsHigh: height + 32, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0),
          let context = NSGraphicsContext(bitmapImageRep: bitmap) else { continue }
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = context
    context.imageInterpolation = .high
    NSColor(calibratedWhite: 0.94, alpha: 1).setFill()
    NSRect(x: 0, y: 0, width: width * 2, height: height + 32).fill()
    left.draw(in: NSRect(x: 0, y: 0, width: width, height: height), from: .zero, operation: .copy, fraction: 1)
    right.draw(in: NSRect(x: width, y: 0, width: width, height: height), from: .zero, operation: .copy, fraction: 1)
    let attributes: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: 12, weight: .medium), .foregroundColor: NSColor(calibratedWhite: 0.2, alpha: 1)]
    ("Native macOS · " + name + " · " + appearance as NSString).draw(at: NSPoint(x: 12, y: height + 9), withAttributes: attributes)
    ("Gedeeld · " + name + " · " + appearance as NSString).draw(at: NSPoint(x: width + 12, y: height + 9), withAttributes: attributes)
    NSGraphicsContext.restoreGraphicsState()
    let folder = output.appendingPathComponent(appearance, isDirectory: true)
    try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
    guard let png = bitmap.representation(using: .png, properties: [:]) else { continue }
    try png.write(to: folder.appendingPathComponent(name + ".png"), options: .atomic)
    count += 1
}
print("Composed \(count) pairs from owned screenshots: \(output.path)")
