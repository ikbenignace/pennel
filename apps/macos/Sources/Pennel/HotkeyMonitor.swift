import ApplicationServices
import CoreGraphics
import Foundation
@preconcurrency import PennelCore
import PennelKit

/// Listens system-wide for the dictation modifier key via a listen-only event tap.
/// All access, including the timer's Sendable callback, stays on the main run loop.
final class HotkeyMonitor: @unchecked Sendable {
    private let onEdge: (KeyEdge, TimeInterval) -> Void
    private let onInterrupt: () -> Void
    private let onShift: (Bool) -> Void
    private let onAvailabilityChange: (Bool) -> Void
    private var trigger: HotkeyTrigger
    private var tap: CFMachPort?
    private var source: CFRunLoopSource?
    private var tapID: UInt32?
    private var healthPoll: Timer?
    private var reportedAvailability: Bool?
    private var isDown = false

    init(
        trigger: HotkeyTrigger,
        onEdge: @escaping (KeyEdge, TimeInterval) -> Void,
        onInterrupt: @escaping () -> Void,
        onShift: @escaping (Bool) -> Void,
        onAvailabilityChange: @escaping (Bool) -> Void = { _ in }
    ) {
        self.trigger = trigger
        self.onEdge = onEdge
        self.onInterrupt = onInterrupt
        self.onShift = onShift
        self.onAvailabilityChange = onAvailabilityChange
    }

    deinit { stop() }

    var isRunning: Bool {
        guard let tap, let tapID else { return false }
        let events = EventTapInspection.list()?.first { $0.eventTapID == tapID }?.eventsOfInterest
        return EventTapHealth(
            authorized: AXIsProcessTrusted(), valid: CFMachPortIsValid(tap),
            enabled: CGEvent.tapIsEnabled(tap: tap), events: events
        ).isAvailable
    }
    var isKeyDown: Bool { isDown }

    func setTrigger(_ trigger: HotkeyTrigger) {
        self.trigger = trigger
        isDown = false
    }

    func start() -> Bool {
        let available = EventTapRecovery.restore(
            isAvailable: { self.isRunning }, authorized: { AXIsProcessTrusted() },
            remove: { self.clearTap() }, create: { self.installTap() }
        )
        if available, healthPoll == nil {
            healthPoll = Timer.scheduledTimer(withTimeInterval: 2, repeats: true) { [weak self] _ in
                guard let self else { return }
                if !self.isRunning { _ = self.start() }
            }
        }
        reportAvailability(available)
        return available
    }

    private func installTap() -> Bool {
        guard let existing = EventTapInspection.list() else { return false }
        let previous = Set(existing.map(\.eventTapID))
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
            eventsOfInterest: EventTapHealth.requiredEvents,
            callback: callback,
            userInfo: Unmanaged.passUnretained(self).toOpaque()
        ) else { return false }
        self.tap = tap
        tapID = EventTapInspection.list().flatMap {
            EventTapInspection.newTapID(previous: previous, current: $0, process: ProcessInfo.processInfo.processIdentifier)
        }
        guard let source = CFMachPortCreateRunLoopSource(nil, tap, 0) else {
            return false
        }
        self.source = source
        CFRunLoopAddSource(CFRunLoopGetMain(), source, .commonModes)
        CGEvent.tapEnable(tap: tap, enable: true)
        return true
    }

    func stop() {
        healthPoll?.invalidate()
        healthPoll = nil
        clearTap()
        reportAvailability(false)
    }

    private func clearTap() {
        let wasTracking = tap != nil
        if let tap {
            CGEvent.tapEnable(tap: tap, enable: false)
            CFMachPortInvalidate(tap)
        }
        if let source { CFRunLoopRemoveSource(CFRunLoopGetMain(), source, .commonModes) }
        tap = nil
        source = nil
        tapID = nil
        if isDown {
            isDown = false
            onInterrupt()
            onEdge(.up, ProcessInfo.processInfo.systemUptime)
        }
        if wasTracking { onShift(false) }
    }

    private func reportAvailability(_ available: Bool) {
        guard reportedAvailability != available else { return }
        reportedAvailability = available
        onAvailabilityChange(available)
    }

    private static let leftShift: Int64 = 56
    private static let rightShift: Int64 = 60

    private func handle(type: CGEventType, event: CGEvent) {
        if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
            if let tap, AXIsProcessTrusted() { CGEvent.tapEnable(tap: tap, enable: true) }
            return
        }
        if type == .keyDown {
            if isDown { onInterrupt() }
            return
        }
        guard type == .flagsChanged else { return }
        let keyCode = event.getIntegerValueField(.keyboardEventKeycode)
        if keyCode == Self.leftShift || keyCode == Self.rightShift {
            onShift(event.flags.contains(.maskShift))
            return
        }
        guard keyCode == trigger.keyCode else { return }
        let down = event.flags.rawValue & trigger.deviceMask != 0
        guard down != isDown else { return }
        isDown = down
        onEdge(down ? .down : .up, ProcessInfo.processInfo.systemUptime)
    }
}

/// `.controlSuper` is a Windows/Linux chord; a Mac falls back to fn for it.
private extension HotkeyTrigger {
    var keyCode: Int64 {
        switch self {
        case .fn, .controlSuper: 63
        case .rightOption: 61
        case .rightCommand: 54
        case .rightControl: 62
        }
    }

    /// Device-dependent modifier bits, so left and right keys are told apart.
    var deviceMask: UInt64 {
        switch self {
        case .fn, .controlSuper: CGEventFlags.maskSecondaryFn.rawValue
        case .rightOption: 0x40
        case .rightCommand: 0x10
        case .rightControl: 0x2000
        }
    }
}
