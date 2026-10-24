import Foundation
@preconcurrency import PennelCore

final class AppleIntelligenceModel: PennelCore.SystemLanguageModel, @unchecked Sendable {
    func complete(messages: [LanguageModelMessage], timeoutSecs: UInt64) throws -> String {
        guard !Thread.isMainThread else {
            throw PennelError.InvalidResponse(message: "Het Apple-taalmodel moet op de achtergrond worden aangeroepen.")
        }
        let request = messages.map { NativeLanguageModelMessage(role: $0.role, content: $0.content) }
        let result = ResultBox()
        let done = DispatchSemaphore(value: 0)
        Task.detached {
            do { result.value = .success(try await FoundationLanguageModel.complete(messages: request, timeoutSecs: timeoutSecs)) }
            catch { result.value = .failure(PennelError.InvalidResponse(message: error.localizedDescription)) }
            done.signal()
        }
        done.wait()
        return try result.value!.get()
    }

    private final class ResultBox: @unchecked Sendable {
        var value: Result<String, Error>?
    }
}
