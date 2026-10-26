import AVFoundation

/// One microphone engine for everything that records at the same time (a meeting, the keyboard session, in-app
/// dictation): each client gets every sample, and the engine runs while at least one client listens (spec §5.3).
final class SharedMicrophone: @unchecked Sendable {
    static let shared = SharedMicrophone(capture: MicCapture(audioSession: nil))

    private let capture: AudioCapturing
    /// Serialises starting and stopping the engine. The realtime callback never takes it: stopping an engine waits
    /// for a callback in flight, which must not be waiting on the same lock.
    private let control = NSLock()
    private let listenersLock = NSLock()
    private var listeners: [ObjectIdentifier: @Sendable ([Float]) -> Void] = [:]
    private var running = false
    private var observers: [NSObjectProtocol] = []

    init(capture: AudioCapturing, notifications: NotificationCenter = .default) {
        self.capture = capture
        observe(notifications)
    }

    func client() -> AudioCapturing {
        Client(microphone: self)
    }

    fileprivate func add(_ client: ObjectIdentifier, _ onSamples: @escaping @Sendable ([Float]) -> Void) throws {
        try control.withLock {
            listenersLock.withLock { listeners[client] = onSamples }
            // After a media-services reset the engine is gone even though nobody stopped it.
            if running, !capture.isRunning {
                capture.stop()
                running = false
            }
            guard !running else { return }
            do {
                try startCapture()
            } catch {
                listenersLock.withLock { listeners[client] = nil }
                throw error
            }
        }
    }

    fileprivate func remove(_ client: ObjectIdentifier) {
        control.withLock {
            let empty = listenersLock.withLock {
                listeners[client] = nil
                return listeners.isEmpty
            }
            guard empty, running else { return }
            running = false
            capture.stop()
        }
    }

    fileprivate func isAlive(_ client: ObjectIdentifier) -> Bool {
        control.withLock {
            running && capture.isRunning && listenersLock.withLock { listeners[client] != nil }
        }
    }

    /// The engine stops by itself when the route or its configuration changes (AirPods pulled, a new input). The
    /// clients still listening get a fresh engine; when that fails they stop receiving samples and notice the stall.
    func restartIfStopped() {
        control.withLock {
            guard running, !capture.isRunning else { return }
            capture.stop()
            running = false
            guard listenersLock.withLock({ !listeners.isEmpty }) else { return }
            try? startCapture()
        }
    }

    private func startCapture() throws {
        try capture.start { [weak self] samples in self?.fan(samples) }
        running = true
    }

    private func observe(_ center: NotificationCenter) {
        let restart: @Sendable (Notification) -> Void = { [weak self] _ in self?.restartIfStopped() }
        observers.append(center.addObserver(forName: .AVAudioEngineConfigurationChange, object: nil, queue: nil, using: restart))
        observers.append(center.addObserver(forName: AVAudioSession.routeChangeNotification, object: nil, queue: nil, using: restart))
        observers.append(center.addObserver(forName: AVAudioSession.mediaServicesWereResetNotification, object: nil, queue: nil, using: restart))
    }

    private func fan(_ samples: [Float]) {
        let targets = listenersLock.withLock { Array(listeners.values) }
        for target in targets { target(samples) }
    }

    private final class Client: AudioCapturing, @unchecked Sendable {
        private let microphone: SharedMicrophone

        init(microphone: SharedMicrophone) {
            self.microphone = microphone
        }

        func start(onSamples: @escaping @Sendable ([Float]) -> Void) throws {
            try microphone.add(ObjectIdentifier(self), onSamples)
        }

        func stop() {
            microphone.remove(ObjectIdentifier(self))
        }

        var isRunning: Bool { microphone.isAlive(ObjectIdentifier(self)) }

        deinit {
            microphone.remove(ObjectIdentifier(self))
        }
    }
}

/// Reference-counted `AVAudioSession` activation, so a meeting ending does not cut the keyboard session's
/// microphone and the other way round. The first holder's category stays until the last one leaves.
final class SharedAudioSession: @unchecked Sendable {
    static let shared = SharedAudioSession()

    private let lock = NSLock()
    private var holders = 0

    func client(_ purpose: SystemAudioSession.Purpose) -> AudioSessionControlling {
        Client(shared: self, purpose: purpose)
    }

    fileprivate func acquire(_ purpose: SystemAudioSession.Purpose) throws {
        try lock.withLock {
            let system = SystemAudioSession(purpose: purpose)
            // After an interruption iOS has deactivated the session even while holders remain, so activating
            // again is always needed; only the category is left alone when someone else already set it.
            if holders == 0 { try system.configure() }
            try system.setActive()
            holders += 1
        }
    }

    fileprivate func release() {
        lock.withLock {
            holders = max(0, holders - 1)
            if holders == 0 { SystemAudioSession(purpose: .dictation).deactivate() }
        }
    }

    private final class Client: AudioSessionControlling, @unchecked Sendable {
        private let shared: SharedAudioSession
        private let purpose: SystemAudioSession.Purpose
        private let lock = NSLock()
        private var holding = false

        init(shared: SharedAudioSession, purpose: SystemAudioSession.Purpose) {
            self.shared = shared
            self.purpose = purpose
        }

        func activate() throws {
            try lock.withLock {
                guard !holding else { return }
                try shared.acquire(purpose)
                holding = true
            }
        }

        func deactivate() {
            lock.withLock {
                guard holding else { return }
                holding = false
                shared.release()
            }
        }
    }
}
