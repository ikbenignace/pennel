// Port of apps/macos-platform/Sources/PennelMacPlatform/FoundationLanguageModel.swift. Keep behavior and canonical English diagnostics in step with it.
import Foundation
#if canImport(FoundationModels)
import FoundationModels
#endif

struct NativeLanguageModelMessage: Codable, Sendable, Equatable {
    let role: String
    let content: String
}

struct NativeLanguageModelAvailability: Sendable, Equatable {
    enum Cause { case deviceNotEligible, disabled, notReady, unknown }
    let status: String
    let reason: String?

    static let ready = Self(status: "available", reason: nil)
    static let unsupported = Self(status: "unsupported", reason: "Apple Intelligence requires iOS 26 or later and a supported iPhone.")

    static func unavailable(_ cause: Cause) -> Self {
        switch cause {
        case .deviceNotEligible:
            Self(status: "unsupported", reason: "This iPhone does not support Apple's on-device language model.")
        case .disabled:
            Self(status: "unavailable", reason: "Enable Apple Intelligence in Settings → Apple Intelligence & Siri.")
        case .notReady:
            Self(status: "unavailable", reason: "The Apple Intelligence model is not ready yet. Wait until iOS has downloaded it and try again.")
        case .unknown:
            Self(status: "unavailable", reason: "The Apple Intelligence model is currently unavailable. Check Apple Intelligence in Settings and try again.")
        }
    }
}

enum NativeLanguageModelError: Error, LocalizedError, Equatable {
    case invalidRequest, inputTooLong, timedOut, cancelled, refused, unsupportedLanguage, unavailable(String), generationFailed, emptyResponse

    var errorDescription: String? {
        switch self {
        case .invalidRequest: "Invalid request for the built-in Apple text model."
        case .inputTooLong: "This text is too long for the built-in Apple text model. Choose another model or shorten the text."
        case .timedOut: "The built-in Apple text model did not respond in time. Try again."
        case .cancelled: "The request to the built-in Apple language model was cancelled."
        case .refused: "The built-in Apple text model could not process this text. Your original text is preserved."
        case .unsupportedLanguage: "The built-in Apple text model does not support this language or region."
        case .unavailable(let reason): reason
        case .generationFailed: "The built-in Apple text model could not produce an answer. Try again or choose another model."
        case .emptyResponse: "The built-in Apple text model returned no text."
        }
    }
}

enum NativeLanguageModelPolicy {
    static let inputByteLimit = 6_000
    static let messageOverheadBytes = 64
    static let reservedResponseTokens = 1_024

    static func validate(inputTokens: Int, contextSize: Int) throws {
        guard inputTokens >= 0, contextSize > reservedResponseTokens,
              inputTokens <= contextSize - reservedResponseTokens else { throw NativeLanguageModelError.inputTooLong }
    }
}

struct NativeLanguageModelRequest: Sendable {
    let messages: [NativeLanguageModelMessage]
    let inputBytes: Int

    init(json: String) throws {
        guard let data = json.data(using: .utf8),
              let messages = try? JSONDecoder().decode([NativeLanguageModelMessage].self, from: data) else {
            throw NativeLanguageModelError.invalidRequest
        }
        try self.init(messages: messages)
    }

    init(messages: [NativeLanguageModelMessage]) throws {
        guard !messages.isEmpty, messages.last?.role == "user" else { throw NativeLanguageModelError.invalidRequest }
        var expectedRole = "user"
        var dialogueStarted = false
        for message in messages {
            guard !message.content.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { throw NativeLanguageModelError.invalidRequest }
            if message.role == "system" && !dialogueStarted { continue }
            dialogueStarted = true
            guard message.role == expectedRole else { throw NativeLanguageModelError.invalidRequest }
            expectedRole = expectedRole == "user" ? "assistant" : "user"
        }
        inputBytes = messages.reduce(0) { $0 + $1.content.utf8.count + NativeLanguageModelPolicy.messageOverheadBytes }
        guard inputBytes <= NativeLanguageModelPolicy.inputByteLimit else { throw NativeLanguageModelError.inputTooLong }
        self.messages = messages
    }
}

enum NativeLanguageModelDeadline {
    static func run(nanoseconds: UInt64, operation: @escaping @Sendable () async throws -> String) async throws -> String {
        let gate = CompletionGate()
        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                gate.attach(continuation)
                let work = Task.detached {
                    do { gate.finish(.success(try await operation())) }
                    catch { gate.finish(.failure(error)) }
                }
                let timer = Task.detached {
                    do { try await Task.sleep(nanoseconds: nanoseconds) }
                    catch { return }
                    gate.finish(.failure(NativeLanguageModelError.timedOut))
                }
                gate.attach(tasks: [work, timer])
            }
        } onCancel: {
            gate.finish(.failure(NativeLanguageModelError.cancelled))
        }
    }

    // A task group would wait for a model that ignores cancellation, delaying its terminal callback.
    private final class CompletionGate: @unchecked Sendable {
        private let lock = NSLock()
        private var continuation: CheckedContinuation<String, Error>?
        private var result: Result<String, Error>?
        private var tasks: [Task<Void, Never>] = []

        func attach(_ continuation: CheckedContinuation<String, Error>) {
            lock.lock()
            if let result {
                lock.unlock()
                continuation.resume(with: result)
            } else {
                self.continuation = continuation
                lock.unlock()
            }
        }

        func attach(tasks: [Task<Void, Never>]) {
            lock.lock()
            let finished = result != nil
            if !finished { self.tasks = tasks }
            lock.unlock()
            if finished { tasks.forEach { $0.cancel() } }
        }

        func finish(_ result: Result<String, Error>) {
            lock.lock()
            guard self.result == nil else { lock.unlock(); return }
            self.result = result
            let continuation = self.continuation
            self.continuation = nil
            let tasks = self.tasks
            self.tasks = []
            lock.unlock()
            tasks.forEach { $0.cancel() }
            continuation?.resume(with: result)
        }
    }
}

enum FoundationLanguageModel {
    static func availability() -> NativeLanguageModelAvailability {
        #if canImport(FoundationModels)
        if #available(iOS 26, *) {
            switch FoundationModels.SystemLanguageModel.default.availability {
            case .available: return .ready
            case .unavailable(.deviceNotEligible): return .unavailable(.deviceNotEligible)
            case .unavailable(.appleIntelligenceNotEnabled): return .unavailable(.disabled)
            case .unavailable(.modelNotReady): return .unavailable(.notReady)
            case .unavailable: return .unavailable(.unknown)
            }
        }
        #endif
        return .unsupported
    }

    static func complete(messages: [NativeLanguageModelMessage], timeoutSecs: UInt64) async throws -> String {
        let request = try NativeLanguageModelRequest(messages: messages)
        let state = availability()
        guard state.status == "available" else { throw NativeLanguageModelError.unavailable(state.reason ?? NativeLanguageModelAvailability.unsupported.reason!) }
        guard timeoutSecs > 0 else { throw NativeLanguageModelError.timedOut }
        let nanoseconds = min(timeoutSecs, 300) * 1_000_000_000
        return try await NativeLanguageModelDeadline.run(nanoseconds: nanoseconds) {
            #if canImport(FoundationModels)
            if #available(iOS 26, *) {
                do { return try await generate(request) }
                catch { throw safeError(error) }
            }
            #endif
            throw NativeLanguageModelError.unavailable(NativeLanguageModelAvailability.unsupported.reason!)
        }
    }

    #if canImport(FoundationModels)
    @available(iOS 26, *)
    static func transcriptEntries(_ messages: [NativeLanguageModelMessage]) -> [Transcript.Entry] {
        messages.map { message in
            let segments: [Transcript.Segment] = [.text(.init(content: message.content))]
            switch message.role {
            case "system": return .instructions(.init(segments: segments, toolDefinitions: []))
            case "assistant": return .response(.init(assetIDs: [], segments: segments))
            default: return .prompt(.init(segments: segments))
            }
        }
    }

    @available(iOS 26, *)
    private static func generate(_ request: NativeLanguageModelRequest) async throws -> String {
        try Task.checkCancellation()
        let model = FoundationModels.SystemLanguageModel.default
        let entries = transcriptEntries(request.messages)
        #if compiler(>=6.3)
        if #available(iOS 26.4, *) {
            let count = try await model.tokenCount(for: entries)
            try NativeLanguageModelPolicy.validate(inputTokens: count, contextSize: model.contextSize)
        }
        #endif
        let session = LanguageModelSession(model: model, tools: [], transcript: Transcript(entries: entries.dropLast()))
        // Leave room for a complete response; a token cap can return silently clipped text.
        #if compiler(>=6.4)
        let options = GenerationOptions(samplingMode: .greedy, temperature: 0)
        #else
        let options = GenerationOptions(sampling: .greedy, temperature: 0)
        #endif
        let response = try await session.respond(to: request.messages.last!.content, options: options)
        try Task.checkCancellation()
        let text = response.content.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { throw NativeLanguageModelError.emptyResponse }
        return text
    }

    @available(iOS 26, *)
    private static func safeError(_ error: Error) -> NativeLanguageModelError {
        if let error = error as? NativeLanguageModelError { return error }
        if error is CancellationError { return .cancelled }
        #if compiler(>=6.4)
        if #available(iOS 27, *), let error = error as? LanguageModelError {
            switch error {
            case .contextSizeExceeded: return .inputTooLong
            case .guardrailViolation, .refusal: return .refused
            case .unsupportedLanguageOrLocale: return .unsupportedLanguage
            case .timeout: return .timedOut
            default: return .generationFailed
            }
        }
        #endif
        if let error = error as? LanguageModelSession.GenerationError {
            switch error {
            case .exceededContextWindowSize: return .inputTooLong
            case .assetsUnavailable: return .unavailable(NativeLanguageModelAvailability.unavailable(.notReady).reason!)
            case .guardrailViolation, .refusal: return .refused
            case .unsupportedLanguageOrLocale: return .unsupportedLanguage
            default: return .generationFailed
            }
        }
        return .generationFailed
    }
    #endif
}

/// The generated `SystemLanguageModel` protocol the core calls for Apple Intelligence.
final class AppleIntelligenceModel: SystemLanguageModel, @unchecked Sendable {
    func complete(messages: [LanguageModelMessage], timeoutSecs: UInt64) throws -> String {
        let native = messages.map { NativeLanguageModelMessage(role: $0.role, content: $0.content) }
        let box = ResultBox()
        let done = DispatchSemaphore(value: 0)
        Task.detached {
            box.result = await Result { try await FoundationLanguageModel.complete(messages: native, timeoutSecs: timeoutSecs) }
            done.signal()
        }
        done.wait()
        do {
            return try box.result!.get()
        } catch {
            throw PennelError.InvalidResponse(message: error.localizedDescription)
        }
    }

    private final class ResultBox: @unchecked Sendable {
        var result: Result<String, Error>?
    }
}

extension FoundationLanguageModel {
    static var isAvailable: Bool { availability().status == "available" }
}
