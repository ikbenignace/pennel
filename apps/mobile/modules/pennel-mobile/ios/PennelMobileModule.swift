import ExpoModulesCore
import Foundation
import UIKit

public final class PennelMobileModule: Module {
    private let observation = ModuleObservation()

    public func definition() -> ModuleDefinition {
        Name("PennelMobile")
        Events("snapshot")

        AsyncFunction("bootstrap") { () async throws -> String in
            do { return try await PennelNativeRuntime.shared.bootstrap() }
            catch { throw Self.failure(error) }
        }
        AsyncFunction("getSnapshot") { () async throws -> String in
            do { return try await PennelNativeRuntime.shared.getSnapshot() }
            catch { throw Self.failure(error) }
        }
        AsyncFunction("invoke") { (method: String, payload: String) async throws -> String in
            do { return try await PennelNativeRuntime.shared.invoke(method: method, payload: payload) }
            catch { throw Self.failure(error) }
        }
        OnStartObserving("snapshot") { [weak self] in
            guard let self, let start = self.observation.begin() else { return }
            let observation = self.observation
            let recipient = WeakModuleRecipient(self)
            Task { @MainActor in
                if let previous = start.previous { PennelNativeRuntime.shared.removeObserver(previous) }
                guard recipient.isAlive else { return }
                let id = PennelNativeRuntime.shared.observe { snapshot in
                    recipient.send(snapshot)
                }
                if !observation.install(id, generation: start.generation) { PennelNativeRuntime.shared.removeObserver(id) }
            }
        }
        OnStopObserving("snapshot") { [weak self] in self?.stopObserving() }
        OnDestroy { [observation = self.observation] in
            if let id = observation.remove(permanently: true) {
                Task { @MainActor in PennelNativeRuntime.shared.removeObserver(id) }
            }
        }
    }

    private func stopObserving(permanently: Bool = false) {
        guard let id = observation.remove(permanently: permanently) else { return }
        Task { @MainActor in PennelNativeRuntime.shared.removeObserver(id) }
    }

    private static func failure(_ error: Error) -> NSError {
        let code: String
        let message: String
        if let value = error as? PennelBridgeError { code = value.code; message = value.message }
        else if let value = error as? PennelError {
            switch value {
            case .Cloud(let cloudCode, let text): code = cloudCode; message = text
            case .NotSignedIn: code = "signed_out"; message = ErrorText.message(value)
            case .VaultLocked: code = "vault_locked"; message = ErrorText.message(value)
            case .Cancelled: code = "cancelled"; message = "Cancelled."
            case .Network: code = "network"; message = ErrorText.message(value)
            case .Storage: code = "storage"; message = ErrorText.message(value)
            case .Http(let status, _):
                code = "http_\(status)"
                message = "The provider could not complete this request. Check its access, usage limit, and selected model."
            default: code = ErrorText.code(value); message = ErrorText.message(value)
            }
        } else { code = "native_error"; message = "The native operation could not complete." }
        let text = (try? BridgeJSON.encode(["code": code, "message": message])) ?? message
        return NSError(domain: "PennelMobile", code: 1, userInfo: [NSLocalizedDescriptionKey: text])
    }
}

// Only this immutable weak reference crosses from Expo's queue to MainActor; the SDK module
// itself is never declared Sendable or accessed by our event delivery outside MainActor.
private final class WeakModuleRecipient: @unchecked Sendable {
    private weak var module: PennelMobileModule?
    init(_ module: PennelMobileModule) { self.module = module }
    @MainActor var isAlive: Bool { module != nil }
    @MainActor func send(_ snapshot: String) { module?.sendEvent("snapshot", ["snapshot": snapshot]) }
}

final class ModuleObservation: @unchecked Sendable {
    private let lock = NSLock()
    private var id: UUID?
    private var destroyed = false
    private var active = false
    private var generation: UInt64 = 0
    func begin() -> (generation: UInt64, previous: UUID?)? {
        lock.withLock {
            guard !destroyed else { return nil }
            generation += 1
            active = true
            defer { id = nil }
            return (generation, id)
        }
    }
    func install(_ id: UUID, generation: UInt64) -> Bool {
        lock.withLock {
            guard !destroyed, active, generation == self.generation else { return false }
            self.id = id
            return true
        }
    }
    func remove(permanently: Bool) -> UUID? {
        lock.withLock {
            if permanently { destroyed = true }
            active = false
            generation += 1
            defer { id = nil }
            return id
        }
    }
}

public final class PennelAppDelegateSubscriber: ExpoAppDelegateSubscriber {
    // Subscriber registration runs inside Objective-C +load. System frameworks can
    // deadlock there on dyld's initializer lock; launch still precedes scene/JS startup.
    public func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil) -> Bool {
        _ = try? PennelNativeRuntime.shared.bootstrap()
        return true
    }

    public func application(_ app: UIApplication, open url: URL, options: [UIApplication.OpenURLOptionsKey: Any] = [:]) -> Bool {
        PennelNativeRuntime.shared.open(url)
    }
}
