import CoreGraphics
import Foundation

/// Global ⌘⇧R that is swallowed, so the app in front (a browser's hard reload) never sees it.
/// Only touched from the main run loop, where the tap and its callback live.
final class ShortcutTap: @unchecked Sendable {
    private static let rKey: Int64 = 15
    private let onTrigger: () -> Void
    private var tap: CFMachPort?
    private var source: CFRunLoopSource?

    init(onTrigger: @escaping () -> Void) {
        self.onTrigger = onTrigger
    }

    deinit { stop() }

    var isRunning: Bool { tap != nil }

    func start() -> Bool {
        guard tap == nil else { return true }
        let callback: CGEventTapCallBack = { _, type, event, refcon in
            guard let refcon else { return Unmanaged.passUnretained(event) }
            let shortcut = Unmanaged<ShortcutTap>.fromOpaque(refcon).takeUnretainedValue()
            if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
                if let tap = shortcut.tap { CGEvent.tapEnable(tap: tap, enable: true) }
                return Unmanaged.passUnretained(event)
            }
            guard shortcut.matches(event) else { return Unmanaged.passUnretained(event) }
            // Return at once: a swallowing tap delays every keystroke on the Mac while it runs.
            if event.getIntegerValueField(.keyboardEventAutorepeat) == 0 {
                DispatchQueue.main.async { shortcut.onTrigger() }
            }
            return nil
        }
        guard let tap = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .defaultTap,
            eventsOfInterest: CGEventMask(1 << CGEventType.keyDown.rawValue),
            callback: callback,
            userInfo: Unmanaged.passUnretained(self).toOpaque()
        ) else { return false }
        self.tap = tap
        source = CFMachPortCreateRunLoopSource(nil, tap, 0)
        CFRunLoopAddSource(CFRunLoopGetMain(), source, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: true)
        return true
    }

    func stop() {
        if let tap { CGEvent.tapEnable(tap: tap, enable: false) }
        if let source { CFRunLoopRemoveSource(CFRunLoopGetMain(), source, .commonModes) }
        tap = nil
        source = nil
    }

    private func matches(_ event: CGEvent) -> Bool {
        let flags = event.flags
        return event.getIntegerValueField(.keyboardEventKeycode) == Self.rKey
            && flags.contains(.maskCommand) && flags.contains(.maskShift)
            && !flags.contains(.maskControl) && !flags.contains(.maskAlternate)
    }
}
