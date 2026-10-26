import Foundation
import PennelMobileKit
import Testing

@MainActor
final class Harness {
    let store = HandoffStore(directory: FileManager.default.temporaryDirectory.appendingPathComponent("pennel-keyboard-\(UUID().uuidString)"))
    let start = Date(timeIntervalSince1970: 1_800_000_000)
    var now: Date
    var inserted: [String] = []
    var opens = 0
    var openSucceeds = true

    init() { now = start }

    func model(fullAccess: Bool = true, pending: PendingHandoff = PendingHandoff()) -> KeyboardModel {
        KeyboardModel(
            hasFullAccess: fullAccess,
            client: HandoffClient(store: store, pending: pending),
            now: { [unowned self] in now },
            openApp: { [unowned self] in opens += 1; return openSucceeds },
            onResult: { [unowned self] in inserted.append($0) })
    }

    func session(_ phase: SessionPhase, heartbeat: Date? = nil) throws {
        try store.write(
            SessionState(phase: phase, expiresAt: now.addingTimeInterval(300), heartbeatAt: heartbeat ?? now, level: 0),
            as: .session)
    }

    var lastCommand: HandoffCommand? { store.read(HandoffCommand.self, as: .command) }

    func answer(_ text: String, to id: UUID) throws {
        try store.write(HandoffResult(id: id, text: text, error: nil, at: now), as: .result)
    }
}

@MainActor
struct KeyboardModelTests {
    let h = Harness()

    @Test func no_full_access_shows_settings_hint_and_letters_still_work() throws {
        try h.session(.ready)
        let model = h.model(fullAccess: false)
        model.refresh()
        #expect(model.pad == .needsFullAccess)
        model.tapPill()
        model.holdBegan()
        #expect(h.lastCommand == nil)
        #expect(h.opens == 0)
        #expect(Copy.Keyboard.fullAccessNeeded.contains("Typen werkt ook zonder"))
        #expect(KeyboardLayout.qwerty.rows.count == 3)
    }

    @Test func session_off_tap_opens_app() {
        let model = h.model()
        model.refresh()
        #expect(model.pad == .sessionOff)
        model.tapPill()
        #expect(h.opens == 1)
        #expect(model.pad == .sessionOff)
        #expect(h.lastCommand == nil)
    }

    @Test func failed_open_shows_the_manual_fallback() {
        h.openSucceeds = false
        let model = h.model()
        model.refresh()
        model.tapPill()
        #expect(model.pad == .openPennelFallback)
    }

    @Test func hold_sends_start_then_stop() throws {
        try h.session(.ready)
        let model = h.model()
        model.refresh()
        model.holdBegan()
        let start = try #require(h.lastCommand)
        #expect(start.action == .start)
        #expect(model.pad == .listening)
        model.holdEnded()
        let stop = try #require(h.lastCommand)
        #expect(stop.action == .stop)
        #expect(stop.id != start.id)
        #expect(model.pad == .processing)
    }

    @Test func tap_tap_sends_start_then_stop() throws {
        try h.session(.ready)
        let model = h.model()
        model.refresh()
        model.tapPill()
        #expect(h.lastCommand?.action == .start)
        #expect(model.pad == .listening)
        model.holdEnded()
        #expect(h.lastCommand?.action == .start)
        model.tapPill()
        #expect(h.lastCommand?.action == .stop)
        #expect(model.pad == .processing)
    }

    @Test func release_after_a_hands_free_start_does_not_stop() throws {
        try h.session(.ready)
        let model = h.model()
        model.refresh()
        model.tapPill()
        model.holdEnded()
        #expect(h.lastCommand?.action == .start)
    }

    @Test func cancel_sends_cancel() throws {
        try h.session(.ready)
        let model = h.model()
        model.refresh()
        model.tapPill()
        model.cancel()
        #expect(h.lastCommand?.action == .cancel)
        #expect(model.pad == .ready)
    }

    @Test func taps_while_processing_send_nothing() throws {
        try h.session(.processing)
        let model = h.model()
        model.refresh()
        #expect(model.pad == .processing)
        model.tapPill()
        model.holdBegan()
        #expect(h.lastCommand == nil)
    }

    @Test func result_inserted_once() throws {
        try h.session(.ready)
        let model = h.model()
        model.refresh()
        model.tapPill()
        model.tapPill()
        let stop = try #require(h.lastCommand)
        try h.answer("hallo wereld", to: stop.id)
        model.refresh()
        model.refresh()
        #expect(h.inserted == ["hallo wereld"])
        #expect(model.pad == .ready)
    }

    @Test func foreign_result_is_never_inserted() throws {
        try h.session(.ready)
        let model = h.model()
        model.refresh()
        model.tapPill()
        model.tapPill()
        try h.answer("van iemand anders", to: UUID())
        model.refresh()
        #expect(h.inserted.isEmpty)
    }

    @Test func error_result_shows_the_message_and_retry_clears_it() throws {
        try h.session(.ready)
        let model = h.model()
        model.refresh()
        model.tapPill()
        model.tapPill()
        let stop = try #require(h.lastCommand)
        try h.store.write(HandoffResult(id: stop.id, text: nil, error: "Geen spraak gehoord.", at: h.now), as: .result)
        model.refresh()
        #expect(model.pad == .failed("Geen spraak gehoord."))
        #expect(h.inserted.isEmpty)
        model.retry()
        #expect(model.pad == .ready)
    }

    @Test func dismiss_and_reappear_offers_last_result_once() throws {
        try h.session(.ready)
        let first = h.model()
        first.refresh()
        first.tapPill()
        first.tapPill()
        let stop = try #require(h.lastCommand)
        try h.answer("gemist", to: stop.id)

        let reappeared = h.model()
        reappeared.refresh()
        #expect(h.inserted.isEmpty)
        #expect(reappeared.lastResult?.text == "gemist")
        reappeared.insertLast()
        reappeared.insertLast()
        #expect(h.inserted == ["gemist"])
        #expect(reappeared.lastResult == nil)

        let third = h.model()
        third.refresh()
        #expect(third.lastResult == nil)
        #expect(h.inserted == ["gemist"])
    }

    @Test func same_process_reappear_inserts_once_without_a_second_offer() throws {
        let pending = PendingHandoff()
        try h.session(.ready)
        let first = h.model(pending: pending)
        first.refresh()
        first.tapPill()
        first.tapPill()
        try h.answer("een keer", to: try #require(h.lastCommand).id)

        let reappeared = h.model(pending: pending)
        reappeared.refresh()
        first.refresh()
        reappeared.refresh()
        #expect(h.inserted == ["een keer"])
        #expect(reappeared.lastResult == nil)
    }

    @Test func stop_in_flight_is_not_undone_by_a_listening_session_and_never_sent_twice() throws {
        try h.session(.ready)
        let model = h.model()
        model.refresh()
        model.tapPill()
        try h.session(.listening)
        model.tapPill()
        let stop = try #require(h.lastCommand)
        #expect(stop.action == .stop)
        model.refresh()
        #expect(model.pad == .processing)
        model.tapPill()
        model.holdEnded()
        #expect(try #require(h.lastCommand).id == stop.id)
        try h.answer("eerste stop", to: stop.id)
        model.refresh()
        #expect(h.inserted == ["eerste stop"])
    }

    @Test func consumed_result_does_not_rearm_processing() throws {
        try h.session(.ready)
        let model = h.model()
        model.refresh()
        model.tapPill()
        model.tapPill()
        let stop = try #require(h.lastCommand)
        try h.session(.processing)
        try h.answer("klaar", to: stop.id)
        model.refresh()
        #expect(h.inserted == ["klaar"])
        try h.session(.ready)
        h.now = h.start.addingTimeInterval(0.2)
        try h.session(.ready)
        model.refresh()
        #expect(model.pad == .ready)
        model.tapPill()
        #expect(h.lastCommand?.action == .start)
    }

    @Test func stale_session_reads_off() throws {
        try h.session(.ready)
        let model = h.model()
        model.refresh()
        #expect(model.pad == .ready)
        h.now = h.start.addingTimeInterval(6)
        model.refresh()
        #expect(model.pad == .sessionOff)
        model.tapPill()
        #expect(h.opens == 1)
        #expect(h.lastCommand == nil)
    }

    @Test func listening_resets_when_the_session_goes_off() throws {
        try h.session(.ready)
        let model = h.model()
        model.refresh()
        model.tapPill()
        h.now = h.start.addingTimeInterval(6)
        model.refresh()
        #expect(model.pad == .sessionOff)
    }

    @Test func a_start_the_app_never_acted_on_does_not_stay_listening() throws {
        try h.session(.ready)
        let model = h.model()
        model.refresh()
        model.tapPill()
        h.now = h.start.addingTimeInterval(2)
        try h.session(.ready)
        model.refresh()
        #expect(model.pad == .ready)
    }
}

struct KeyboardLayoutTests {
    @Test func azerty_swaps_the_letters_qwerty_keeps() {
        #expect(KeyboardLayout.qwerty.rows[0].first == "q")
        #expect(KeyboardLayout.azerty.rows[0].first == "a")
        #expect(KeyboardLayout.azerty.rows.count == 3)
    }

    @Test func stored_choice_defaults_to_qwerty_and_ignores_garbage() {
        let defaults = UserDefaults(suiteName: "pennel-layout-\(UUID().uuidString)")!
        #expect(KeyboardLayout.stored(in: defaults) == .qwerty)
        KeyboardLayout.azerty.store(in: defaults)
        #expect(KeyboardLayout.stored(in: defaults) == .azerty)
        defaults.set("dvorak", forKey: KeyboardLayout.defaultsKey)
        #expect(KeyboardLayout.stored(in: defaults) == .qwerty)
    }
}
