import AppKit

@main
enum MusliApp {
    static func main() {
        let app = NSApplication.shared
        app.setActivationPolicy(.accessory)
        app.run()
    }
}
