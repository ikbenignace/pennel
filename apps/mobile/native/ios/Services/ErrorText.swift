import Foundation

/// Canonical diagnostics and stable presentation codes never carry user content into log categories.
enum ErrorText {
    static func message(_ error: Error) -> String {
        switch error as? PennelError {
        case .NoSpeech: "No speech recognized. Try again."
        case .MissingApiKey: "No API key configured"
        case .Http(let status, _) where status == 401 || status == 403: "API key rejected"
        case .Http(let status, let message): "HTTP \(status): \(message)"
        case .Network: "No connection"
        case .InvalidResponse(let message), .Storage(let message), .Cloud(_, let message): message
        case .ModelNotDownloaded: "Model not downloaded"
        case .NotSignedIn: "Sign in to Pennel Cloud in Settings → Account."
        case .VaultLocked: "Enter your recovery code to enable sync."
        default: "Failed"
        }
    }

    static func failed(_ error: Error) -> String {
        "Failed: \(message(error))"
    }

    static func failureCode(_ error: Error) -> String {
        switch error as? PennelError {
        case .NoSpeech: "no_speech"
        case .MissingApiKey: "missing_api_key"
        case .Http(let status, _) where status == 401 || status == 403: "provider_auth"
        case .Http(let status, _): "http_\(status)"
        case .Network: "network"
        case .Storage: "storage"
        case .Cloud(let code, _): code
        case .ModelNotDownloaded: "model_not_downloaded"
        case .NotSignedIn: "not_signed_in"
        case .VaultLocked: "vault_locked"
        case .Cancelled: "cancelled"
        case .NotFound: "not_found"
        case .UnsupportedProcessor: "unsupported"
        default: "operation_failed"
        }
    }

    static func code(_ error: Error) -> String {
        if let error = error as? PennelError {
            if case .Cloud = error { return "cloud" }
            return failureCode(error)
        }
        let error = error as NSError
        return "\(error.domain)#\(error.code)"
    }
}
