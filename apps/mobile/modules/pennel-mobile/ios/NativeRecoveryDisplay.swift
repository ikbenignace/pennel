import Foundation

struct NativeRecoveryDisplay {
    let code: String
    let token = UUID().uuidString
    let context: String
    let identity: UInt64

    func matches(token: String, identity: UInt64) -> Bool {
        self.token == token && self.identity == identity
    }

    func payload(epoch: String) -> [String: Any] {
        ["recovery_code": code, "confirmation_token": token, "account_epoch": epoch]
    }

    static func empty(epoch: String) -> [String: Any] {
        ["recovery_code": NSNull(), "confirmation_token": NSNull(), "account_epoch": epoch]
    }
}
