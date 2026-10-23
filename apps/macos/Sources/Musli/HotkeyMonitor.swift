import CoreGraphics
import Foundation
@preconcurrency import MusliCore
import MusliKit

/// Listens system-wide for the dictation modifier key via a listen-only event tap.
final class HotkeyMonitor {
    private let onEdge: (KeyEdge, TimeInterval) -> Void
    private var trigger: HotkeyTrigger
    private var tap: CFMachPort?
    private var source: CFRunLoopSource?
    private var isDown = false

    init(trigger: HotkeyTrigger, onEdge: @escaping (KeyEdge, TimeInterval) -> Void) {
        self.trigger = trigger
        self.onEdge = onEdge
    }

    deinit { stop() }

    var isRunning: Bool { tap != nil }

    func setTrigger(_ trigger: HotkeyTrigger) {
        self.trigger = trigger
        isDown = false
    }

    func start() -> Bool {
        guard tap == nil else { return true }
        let mask = CGEventMask(1 << CGEventType.flagsChanged.rawValue)
        let callback: CGEventTapCallBack = { _, type, event, refcon in
            guard let refcon else { return Unmanaged.passUnretained(event) }
            let monitor = Unmanaged<HotkeyMonitor>.fromOpaque(refcon).takeUnretainedValue()
            monitor.handle(type: type, event: event)
            return Unmanaged.passUnretained(event)
        }
        guard let tap = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .listenOnly,
            eventsOfInterest: mask,
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

    private func handle(type: CGEventType, event: CGEvent) {
        if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
            if let tap { CGEvent.tapEnable(tap: tap, enable: true) }
            return
        }
        guard type == .flagsChanged,
              event.getIntegerValueField(.keyboardEventKeycode) == trigger.keyCode
        else { return }
        let down = event.flags.rawValue & trigger.deviceMask != 0
        guard down != isDown else { return }
        isDown = down
        onEdge(down ? .down : .up, ProcessInfo.processInfo.systemUptime)
    }
}

private extension HotkeyTrigger {
    var keyCode: Int64 {
        switch self {
        case .fn: 63
        case .rightOption: 61
        case .rightCommand: 54
        }
    }

    /// Device-dependent modifier bits, so left and right keys are told apart.
    var deviceMask: UInt64 {
        switch self {
        case .fn: CGEventFlags.maskSecondaryFn.rawValue
        case .rightOption: 0x40
        case .rightCommand: 0x10
        }
    }
}
