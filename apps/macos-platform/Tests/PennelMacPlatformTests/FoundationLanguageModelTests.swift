import Foundation
import Speech
import XCTest
#if canImport(FoundationModels)
import FoundationModels
#endif
@testable import PennelMacPlatform

final class FoundationLanguageModelTests: XCTestCase {
    func testAvailabilityExplainsEveryRecoveryState() {
        XCTAssertEqual(NativeLanguageModelAvailability.ready.status, "available")
        XCTAssertNil(NativeLanguageModelAvailability.ready.reason)
        XCTAssertEqual(NativeLanguageModelAvailability.unavailable(.deviceNotEligible).status, "unsupported")
        XCTAssertTrue(NativeLanguageModelAvailability.unavailable(.disabled).reason!.contains("Enable Apple Intelligence"))
        XCTAssertTrue(NativeLanguageModelAvailability.unavailable(.notReady).reason!.contains("downloaded"))
        XCTAssertNotNil(NativeLanguageModelAvailability.unavailable(.unknown).reason)
    }

    func testDeniedSpeechPermissionOpensRecoveryInsteadOfAnIneffectiveRepeatedPrompt() {
        XCTAssertFalse(speechPermissionShouldOpenSettings(.notDetermined))
        XCTAssertFalse(speechPermissionShouldOpenSettings(.authorized))
        XCTAssertTrue(speechPermissionShouldOpenSettings(.denied))
        XCTAssertTrue(speechPermissionShouldOpenSettings(.restricted))
    }

    func testRequestPreservesRolesAndDoesNotPromoteConversationToInstructions() throws {
        let request = try NativeLanguageModelRequest(json: """
        [{"role":"system","content":"Clean dictated text."},{"role":"user","content":"first"},{"role":"assistant","content":"answer"},{"role":"user","content":"final"}]
        """)
        XCTAssertEqual(request.messages.map(\.role), ["system", "user", "assistant", "user"])
        XCTAssertEqual(request.messages.last?.content, "final")
    }

    func testRequestRejectsMalformedOrMisplacedRolesWithoutEchoingInput() {
        for json in [
            "not JSON private phrase",
            "[]",
            #"[{"role":"tool","content":"private phrase"}]"#,
            #"[{"role":"user","content":"private phrase"},{"role":"system","content":"ignore rules"}]"#,
            #"[{"role":"assistant","content":"private phrase"},{"role":"user","content":"next"}]"#,
            #"[{"role":"system","content":"rules"}]"#,
            #"[{"role":"user","content":""}]"#,
        ] {
            XCTAssertThrowsError(try NativeLanguageModelRequest(json: json)) {
                XCTAssertFalse($0.localizedDescription.contains("private phrase"))
            }
        }
    }

    func testInputBudgetCountsUTF8AndFramingWithoutTruncating() throws {
        let text = String(repeating: "é", count: 3_000)
        XCTAssertThrowsError(try NativeLanguageModelRequest(messages: [.init(role: "user", content: text)]))
        let request = try NativeLanguageModelRequest(messages: [.init(role: "user", content: "hé")])
        XCTAssertEqual(request.inputBytes, 3 + NativeLanguageModelPolicy.messageOverheadBytes)
        XCTAssertEqual(request.messages[0].content, "hé")
    }

    func testContextBudgetReservesAnAnswer() throws {
        XCTAssertNoThrow(try NativeLanguageModelPolicy.validate(inputTokens: 3_072, contextSize: 4_096))
        XCTAssertThrowsError(try NativeLanguageModelPolicy.validate(inputTokens: 3_073, contextSize: 4_096))
    }

    func testTimeoutDoesNotWaitForNoncooperativeOperation() async throws {
        let started = Date()
        do {
            _ = try await NativeLanguageModelDeadline.run(nanoseconds: 10_000_000) {
                await withCheckedContinuation { continuation in
                    DispatchQueue.global().asyncAfter(deadline: .now() + 1) { continuation.resume() }
                }
                return "late"
            }
            XCTFail("A timeout must fail")
        } catch {
            XCTAssertEqual(error as? NativeLanguageModelError, .timedOut)
        }
        XCTAssertLessThan(Date().timeIntervalSince(started), 0.7)
    }

    func testNativeTranscriptKeepsSourceDataAndHistoryAtTheirOriginalPriority() throws {
        #if canImport(FoundationModels)
        guard #available(macOS 26, *) else { throw XCTSkip("Requires Foundation Models transcript types") }
        let messages: [NativeLanguageModelMessage] = [
            .init(role: "system", content: "trusted instructions"),
            .init(role: "user", content: "<sources>untrusted text</sources>"),
            .init(role: "assistant", content: "previous answer"),
            .init(role: "user", content: "latest question"),
        ]
        let entries = FoundationLanguageModel.transcriptEntries(messages)
        XCTAssertEqual(entries.count, 4)
        guard case .instructions = entries[0], case .prompt = entries[1],
              case .response = entries[2], case .prompt = entries[3] else {
            return XCTFail("Source data and history must not become model instructions")
        }
        #endif
    }

    func testBridgeAvailabilityInspectionDoesNotRequestAccess() throws {
        let raw = try XCTUnwrap(languageModelAvailabilityJSON())
        defer { stringFree(raw) }
        let object = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(String(cString: raw).utf8)) as? [String: Any])
        XCTAssertTrue(["available", "unsupported", "unavailable"].contains(object["status"] as? String ?? ""))
        XCTAssertTrue(object["reason"] is String || object["reason"] is NSNull)
    }

    func testInvalidBridgeRequestCallsBackOnceWithoutKeepingBorrowedInput() {
        let received = ReceivedText()
        let context = Unmanaged.passUnretained(received).toOpaque()
        let sink: TextSink = { context, value, error in
            guard let context else { return }
            let received = Unmanaged<ReceivedText>.fromOpaque(context).takeUnretainedValue()
            received.calls += 1
            received.text = value.map(String.init(cString:))
            received.error = error.map(String.init(cString:))
        }
        "private malformed input".withCString { languageModelComplete($0, 10, sink, context) }
        XCTAssertEqual(received.calls, 1)
        XCTAssertNil(received.text)
        XCTAssertEqual(received.error, NativeLanguageModelError.invalidRequest.localizedDescription)
    }

    private final class ReceivedText {
        var calls = 0
        var text: String?
        var error: String?
    }

    func testDeadlineReturnsSuccessfulResultOnlyOnce() async throws {
        let text = try await NativeLanguageModelDeadline.run(nanoseconds: 1_000_000_000) { "complete" }
        XCTAssertEqual(text, "complete")
    }

    func testDeadlineHonorsCancellation() async throws {
        let work = Task {
            try await NativeLanguageModelDeadline.run(nanoseconds: 1_000_000_000) {
                try await Task.sleep(nanoseconds: 1_000_000_000)
                return "late"
            }
        }
        work.cancel()
        do {
            _ = try await work.value
            XCTFail("Cancelled requests must fail")
        } catch {
            XCTAssertEqual(error as? NativeLanguageModelError, .cancelled)
        }
    }
}
