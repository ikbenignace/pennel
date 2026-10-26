import Foundation

/// The desktop's `formatUsage` and `relativeTime` (apps/desktop/src/cloud-login.ts).
enum CloudFormat {
    struct Usage: Equatable {
        let audio: String
        let tokens: String
        let storage: String
    }

    static func usage(_ account: CloudAccount) -> Usage {
        Usage(
            audio: "\(duration(account.audioSeconds)) van \(duration(account.limitAudioSeconds))",
            tokens: "\(millions(account.tokens)) van \(millions(account.limitTokens)) tokens",
            storage: "\(size(account.storageBytes)) van \(size(account.limitStorageBytes))")
    }

    static func relativeTime(thenMs: Int64, nowMs: Int64) -> String {
        let minutes = max(0, (nowMs - thenMs) / 60_000)
        if minutes < 1 { return "zojuist" }
        if minutes < 60 { return "\(minutes) min geleden" }
        let hours = minutes / 60
        if hours < 24 { return "\(hours) u geleden" }
        let days = hours / 24
        return days == 1 ? "1 dag geleden" : "\(days) dagen geleden"
    }

    private static func decimal(_ value: Double) -> String {
        let rounded = (value * 10).rounded() / 10
        return (rounded == rounded.rounded() ? String(Int(rounded)) : String(rounded)).replacingOccurrences(of: ".", with: ",")
    }

    private static func duration(_ seconds: UInt64) -> String {
        let minutes = Int(seconds / 60)
        let hours = minutes / 60
        let rest = minutes % 60
        if hours == 0 { return "\(rest) min" }
        return rest > 0 ? "\(hours) u \(rest) min" : "\(hours) u"
    }

    private static func millions(_ tokens: UInt64) -> String {
        tokens == 0 ? "0 mln" : "\(decimal(Double(tokens) / 1e6)) mln"
    }

    private static func size(_ bytes: UInt64) -> String {
        bytes >= 1_000_000_000 ? "\(decimal(Double(bytes) / 1e9)) GB" : "\(Int((Double(bytes) / 1e6).rounded())) MB"
    }
}
