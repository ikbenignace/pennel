import Foundation

/// Dutch text for the user, and a code for logs that never carries paths, names or content.
enum ErrorText {
    static func message(_ error: Error) -> String {
        switch error as? PennelError {
        case .NoSpeech: "Geen spraak herkend. Probeer opnieuw."
        case .MissingApiKey: "Geen API-sleutel ingesteld"
        case .Http(let status, _) where status == 401 || status == 403: "API-sleutel geweigerd"
        case .Http(let status, let message): "HTTP \(status): \(message)"
        case .Network: "Geen verbinding"
        case .InvalidResponse(let message), .Storage(let message), .Cloud(_, let message): message
        case .ModelNotDownloaded: "Model nog niet gedownload"
        case .NotSignedIn: "Log in bij Pennel Cloud in Instellingen → Account."
        case .VaultLocked: "Voer je herstelcode in om sync aan te zetten."
        default: "Mislukt"
        }
    }

    static func failed(_ error: Error) -> String {
        "Mislukt: \(message(error))"
    }

    static func code(_ error: Error) -> String {
        if error is PennelError {
            return Mirror(reflecting: error).children.first?.label ?? String(describing: error)
        }
        let error = error as NSError
        return "\(error.domain)#\(error.code)"
    }
}
