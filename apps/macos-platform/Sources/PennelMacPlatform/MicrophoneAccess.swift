@preconcurrency import AVFoundation
import Foundation

enum MicrophoneAccessState: Equatable {
    case granted, notDetermined, denied

    init(_ status: AVAuthorizationStatus) {
        switch status {
        case .authorized: self = .granted
        case .notDetermined: self = .notDetermined
        default: self = .denied
        }
    }
}

/// User-facing microphone start failures. CoreAudio's own text is never shown; only its code is logged.
enum MicrophoneStartError: LocalizedError, Equatable {
    case permissionDenied
    case permissionPending
    case failed

    var errorDescription: String? {
        switch self {
        case .permissionDenied: "Geef Pennel toegang tot de microfoon in Systeeminstellingen → Privacy en beveiliging → Microfoon en probeer opnieuw."
        case .permissionPending: "Sta microfoontoegang toe in de melding van macOS en probeer opnieuw."
        case .failed: "De microfoon kon niet starten. Kies een ander invoerapparaat of probeer opnieuw."
        }
    }

    /// AVAudioEngine fails with kAudioUnitErr_FailedInitialization (-10875) when macOS withholds the
    /// microphone, e.g. after an approval stopped matching the app's signature.
    static func forEngineFailure(access: MicrophoneAccessState) -> MicrophoneStartError {
        access == .granted ? .failed : .permissionDenied
    }
}

enum MicrophoneAccess {
    private static let log = Diagnostics("microphone")

    static var state: MicrophoneAccessState { MicrophoneAccessState(AVCaptureDevice.authorizationStatus(for: .audio)) }

    /// Called on the main queue, so an undecided permission is requested without waiting for the answer.
    static func ensureGranted() throws {
        switch state {
        case .granted: return
        case .denied: throw MicrophoneStartError.permissionDenied
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .audio) { _ in }
            throw MicrophoneStartError.permissionPending
        }
    }

    static func start(_ engine: AVAudioEngine) throws {
        do {
            try engine.start()
        } catch {
            let failure = error as NSError
            log.error("microphone start failed: \(failure.domain) \(failure.code)")
            throw MicrophoneStartError.forEngineFailure(access: state)
        }
    }
}
