import AppKit
import ApplicationServices
import Foundation

public struct ExternalPasteSnapshot: Codable, Equatable, Sendable {
    public let token: String
    public let name: String
    public let bundleID: String
}

struct ExternalPasteIdentity: Equatable, Sendable {
    let pid: pid_t
    let launched: Date
    let bundleID: String
}

struct ExternalPasteGuard: Sendable {
    let identity: ExternalPasteIdentity
    var currentOnly = false
    private(set) var cancelled = false

    mutating func observeActivation(pid: pid_t, ownPID: pid_t) {
        if pid != identity.pid && (currentOnly || pid != ownPID) { cancelled = true }
    }

    func accepts(_ current: ExternalPasteIdentity) -> Bool { !cancelled && identity == current }

    func canPaste(foregroundPID: pid_t?, ownPID: pid_t) -> Bool {
        !cancelled && (foregroundPID == identity.pid || (!currentOnly && foregroundPID == ownPID))
    }
}

public enum ExternalPasteError: LocalizedError {
    case unavailable, cancelled, permission, activation, clipboard

    public var errorDescription: String? {
        switch self {
        case .unavailable: "The destination app is no longer available. Your text is in Pennel."
        case .cancelled: "External paste was cancelled. Your text is in Pennel."
        case .permission: "Give Pennel Accessibility access to paste into other apps."
        case .activation: "The destination app could not be activated. Your text is in Pennel."
        case .clipboard: "The clipboard could not be updated. Your text is in Pennel."
        }
    }
}

/// Only opaque, process-bound tokens cross the UI boundary. A changed target never becomes a new recipient.
@MainActor
public final class ExternalPasteCoordinator {
    public static let shared = ExternalPasteCoordinator()

    private struct Target {
        let application: NSRunningApplication
        var guardState: ExternalPasteGuard
        var focusedElement: AXUIElement?
    }

    private var observer: NSObjectProtocol?
    private var latest: Target?
    private var targets: [String: Target] = [:]

    private init() {}

    public func startTracking() {
        guard observer == nil else { return }
        if let app = NSWorkspace.shared.frontmostApplication { activated(app) }
        observer = NSWorkspace.shared.notificationCenter.addObserver(
            forName: NSWorkspace.didActivateApplicationNotification, object: nil, queue: .main
        ) { [weak self] notification in
            guard let app = notification.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication else { return }
            MainActor.assumeIsolated { self?.activated(app) }
        }
    }

    public func capture() -> ExternalPasteSnapshot? {
        startTracking()
        guard var target = latest, valid(target) else { return nil }
        target.guardState = ExternalPasteGuard(identity: target.guardState.identity)
        let token = UUID().uuidString
        targets[token] = target
        return ExternalPasteSnapshot(token: token, name: target.application.localizedName ?? target.guardState.identity.bundleID, bundleID: target.guardState.identity.bundleID)
    }

    public func captureCurrentExternal() -> ExternalPasteSnapshot? {
        startTracking()
        guard let app = NSWorkspace.shared.frontmostApplication, isExternal(app, regularOnly: false),
              let launched = app.launchDate, let bundleID = app.bundleIdentifier else { return nil }
        let identity = ExternalPasteIdentity(pid: app.processIdentifier, launched: launched, bundleID: bundleID)
        let target = Target(application: app, guardState: ExternalPasteGuard(identity: identity, currentOnly: true),
                            focusedElement: focusedElement(pid: app.processIdentifier))
        let token = UUID().uuidString
        targets[token] = target
        return ExternalPasteSnapshot(token: token, name: app.localizedName ?? bundleID, bundleID: bundleID)
    }

    public func cancel(_ token: String) { targets.removeValue(forKey: token) }

    public func invalidateAll() { targets.removeAll() }

    public func isCurrent(_ token: String) -> Bool {
        guard AXIsProcessTrusted(), let target = try? checked(token),
              NSWorkspace.shared.frontmostApplication?.processIdentifier == target.application.processIdentifier else { return false }
        guard let expected = target.focusedElement else { return true }
        guard let current = focusedElement(pid: target.application.processIdentifier) else { return false }
        return CFEqual(expected, current)
    }

    public func paste(text: String, token: String) async throws {
        guard AXIsProcessTrusted() else { throw ExternalPasteError.permission }
        let target = try checked(token)
        let pid = target.application.processIdentifier
        let ownPID = ProcessInfo.processInfo.processIdentifier
        let foreground = NSWorkspace.shared.frontmostApplication?.processIdentifier
        guard target.guardState.canPaste(foregroundPID: foreground, ownPID: ownPID) else {
            cancel(token)
            throw ExternalPasteError.cancelled
        }
        if foreground != pid {
            NSApp.yieldActivation(to: target.application)
            target.application.activate(from: .current, options: [])
            let deadline = ProcessInfo.processInfo.systemUptime + 1
            while NSWorkspace.shared.frontmostApplication?.processIdentifier != pid {
                _ = try checked(token)
                guard ProcessInfo.processInfo.systemUptime < deadline else {
                    cancel(token)
                    throw ExternalPasteError.activation
                }
                try await Task.sleep(for: .milliseconds(20))
            }
        }
        _ = try checked(token)
        guard AXIsProcessTrusted(), NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else {
            throw ExternalPasteError.cancelled
        }
        if let expected = target.focusedElement {
            guard let current = focusedElement(pid: pid), CFEqual(expected, current) else {
                cancel(token)
                throw ExternalPasteError.cancelled
            }
        }
        let source = CGEventSource(stateID: .combinedSessionState)
        guard let down = CGEvent(keyboardEventSource: source, virtualKey: 9, keyDown: true),
              let up = CGEvent(keyboardEventSource: source, virtualKey: 9, keyDown: false) else {
            throw ExternalPasteError.unavailable
        }
        let pasteboard = NSPasteboard.general
        let previous = (pasteboard.pasteboardItems ?? []).map { item in
            Dictionary(uniqueKeysWithValues: item.types.compactMap { type in item.data(forType: type).map { (type, $0) } })
        }
        pasteboard.clearContents()
        guard pasteboard.setString(text, forType: .string) else { throw ExternalPasteError.clipboard }
        pasteboard.setData(Data(), forType: NSPasteboard.PasteboardType("org.nspasteboard.TransientType"))
        let written = pasteboard.changeCount
        down.flags = .maskCommand
        up.flags = .maskCommand
        // Target-addressed events cannot follow a foreground race into an unrelated application.
        down.postToPid(pid)
        up.postToPid(pid)
        try? await Task.sleep(for: .milliseconds(700))
        if pasteboard.changeCount == written {
            let items = previous.map { data in
                let item = NSPasteboardItem()
                for (type, value) in data { item.setData(value, forType: type) }
                return item
            }
            pasteboard.clearContents()
            if !items.isEmpty { pasteboard.writeObjects(items) }
        }
        cancel(token)
    }

    private func checked(_ token: String) throws -> Target {
        guard let target = targets[token], !target.guardState.cancelled else { throw ExternalPasteError.cancelled }
        guard valid(target) else { throw ExternalPasteError.unavailable }
        return target
    }

    private func valid(_ target: Target) -> Bool {
        guard !target.application.isTerminated,
              let current = NSRunningApplication(processIdentifier: target.guardState.identity.pid),
              let launched = current.launchDate, let bundleID = current.bundleIdentifier else { return false }
        return target.guardState.accepts(ExternalPasteIdentity(pid: current.processIdentifier, launched: launched, bundleID: bundleID))
    }

    private func activated(_ app: NSRunningApplication) {
        for token in targets.keys {
            targets[token]?.guardState.observeActivation(pid: app.processIdentifier, ownPID: ProcessInfo.processInfo.processIdentifier)
        }
        guard isExternal(app), let launched = app.launchDate, let bundleID = app.bundleIdentifier else { return }
        latest = Target(application: app, guardState: ExternalPasteGuard(identity: ExternalPasteIdentity(pid: app.processIdentifier, launched: launched, bundleID: bundleID)))
    }

    private func focusedElement(pid: pid_t) -> AXUIElement? {
        var value: CFTypeRef?
        guard AXUIElementCopyAttributeValue(AXUIElementCreateApplication(pid), kAXFocusedUIElementAttribute as CFString, &value) == .success,
              let value, CFGetTypeID(value) == AXUIElementGetTypeID() else { return nil }
        return (value as! AXUIElement)
    }

    private func isExternal(_ app: NSRunningApplication, regularOnly: Bool = true) -> Bool {
        guard app.processIdentifier != ProcessInfo.processInfo.processIdentifier,
              (!regularOnly || app.activationPolicy == .regular), !app.isTerminated else { return false }
        let bundle = app.bundleIdentifier ?? ""
        return !["dev.pennel.", "app.pennel.", "dev.musli."].contains { bundle.hasPrefix($0) }
    }
}
