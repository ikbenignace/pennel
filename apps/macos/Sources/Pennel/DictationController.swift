import AppKit
@preconcurrency import PennelCore
import PennelKit

private final class DictationPreviewForwarder: DictationPreviewObserver, @unchecked Sendable {
    private let onUpdate: @Sendable (DictationPreviewUpdate) -> Void

    init(onUpdate: @escaping @Sendable (DictationPreviewUpdate) -> Void) {
        self.onUpdate = onUpdate
    }

    func changed(update: DictationPreviewUpdate) { onUpdate(update) }
}

struct DictationPreviewOwnership {
    private enum Owner { case external, session(String) }
    private var owners: [String: Owner] = [:]

    mutating func register(_ id: String, owner: String?) {
        owners[id] = owner.map(Owner.session) ?? .external
    }

    func accepts(_ id: String, currentSession: String?) -> Bool {
        switch owners[id] {
        case .external: true
        case .session(let owner): owner == currentSession
        case nil: false
        }
    }

    func recordings(ownedBy owner: String) -> [String] {
        owners.compactMap { id, value in
            if case .session(let session) = value, session == owner { id } else { nil }
        }
    }

    mutating func remove(_ id: String) { owners.removeValue(forKey: id) }
}

enum DictationSelectionSnapshot: Equatable, Sendable {
    case changed
    case current(String?)

    @MainActor static func read(
        isCurrent: @MainActor () -> Bool,
        selection: @MainActor () async -> String?
    ) async -> Self {
        guard isCurrent() else { return .changed }
        let value = await selection()
        guard isCurrent() else { return .changed }
        return .current(value)
    }
}

enum DictationApplicationContext {
    static func bundleID(owned: Bool, atStart: String?, atStop: String?) -> String? {
        owned ? atStart : atStop
    }
}

@MainActor
final class DictationController: OwnedDictationManaging {
    private static let noticeDuration: Duration = .seconds(2)

    private let state: AppState
    private let pill = PillPanel()
    private let recorder = AudioRecorder()
    private let queue = PasteQueue<Delivery>()
    private let log = Diagnostics("dictation")
    private var gesture = HotkeyGesture()
    private var command = CommandIntent()
    private var status = DictationStatus()
    private var monitor: HotkeyMonitor?
    private var ticker: Timer?
    private var noticeTask: Task<Void, Never>?
    private var ownedApplicationID: String?
    private var isRecording = false
    private var insertion: Task<Void, Never>?
    private var ownedCapture: OwnedCapture?
    private var pendingWork = 0
    private var pendingDeliveries = 0
    private var ownedTargets: [String: Set<String>] = [:]
    private var recordingLive: LiveDictationSession?
    private var systemPreview: LiveSpeechPreview?
    private var liveSessions: [String: LiveDictationSession] = [:]
    private var previewOwnership = DictationPreviewOwnership()
    private var focusObserver: NSObjectProtocol?
    weak var meeting: MeetingController?

    init(state: AppState) {
        self.state = state
        focusObserver = NotificationCenter.default.addObserver(forName: NSApplication.didResignActiveNotification, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self, let session = self.state.workspace.session, session.kind == .practice else { return }
                self.closeSession(session.id)
            }
        }
    }

    private struct OwnedCapture: Sendable {
        let sessionId: String
        let kind: OwnedDictationKind
        let externalToken: String?
    }

    private enum Delivery: Sendable {
        case external(String, ExternalPasteSnapshot?)
        case owned(String, OwnedCapture)
    }

    @discardableResult
    func openSession(kind: OwnedDictationKind) -> OwnedDictationSession? {
        if let current = state.workspace.session, current.kind == kind {
            if kind == .pad, !current.busy, pendingDeliveries == 0 { current.targetName = previewTargetName() }
            return current
        }
        guard !isRecording, pendingWork == 0, pendingDeliveries == 0 else {
            state.lastError = "Wacht tot het huidige dictaat is afgerond."
            return nil
        }
        if kind == .practice, meeting?.isRecording == true {
            state.lastError = "Rond eerst je meetingopname af om te oefenen."
            return nil
        }
        if let old = state.workspace.session { closeSession(old.id) }
        let session = OwnedDictationSession(kind: kind)
        if kind == .pad {
            session.text = state.workspace.padText
            session.targetName = previewTargetName()
        }
        state.workspace.session = session
        if state.lastError == "Wacht tot het huidige dictaat is afgerond." || state.lastError == "Rond eerst je meetingopname af om te oefenen." { state.lastError = nil }
        return session
    }

    func toggleSession(_ id: String) {
        guard let session = state.workspace.session, session.id == id else { return }
        if session.phase == .listening { submitSession(id) }
        else if session.phase != .working { beginSession(id) }
    }

    func beginSession(_ id: String) {
        startOwnedSession(id, manual: true)
    }

    private func startOwnedSession(_ id: String, manual: Bool) {
        guard let session = state.workspace.session, session.id == id, !session.busy,
              !isRecording, pendingWork == 0, pendingDeliveries == 0 else { return }
        guard ownsForeground(session) else { return }
        guard session.kind != .practice || meeting?.isRecording != true else {
            session.error = "Rond eerst je meetingopname af om te oefenen."
            session.phase = .error
            return
        }
        guard state.dictationSetup.canRecord else {
            session.error = state.dictationSetup.reason ?? "Kies eerst een werkende spraakherkenning."
            session.phase = .error
            return
        }
        if session.kind == .pad && !state.workspace.flushPad() {
            session.error = state.workspace.padError
            session.phase = .error
            return
        }
        var external: ExternalPasteSnapshot?
        cancelTargets(id)
        if session.kind == .pad, state.settings.dictationPadAutoPaste, Permissions.accessibilityTrusted {
            external = ExternalPasteCoordinator.shared.capture()
            if let token = external?.token { ownedTargets[id, default: []].insert(token) }
        }
        session.targetName = external?.name
        session.error = nil
        session.phase = .idle
        ownedCapture = OwnedCapture(sessionId: id, kind: session.kind, externalToken: external?.token)
        if manual { perform(gesture.manualToggle(at: ProcessInfo.processInfo.systemUptime)) }
        else { beginRecording() }
        if !isRecording {
            ownedCapture = nil
            cancelTargets(id)
            session.phase = .error
            session.error = "Microfoon starten lukt niet. Controleer de toegang en het invoerapparaat."
        }
    }

    func submitSession(_ id: String) {
        guard ownedCapture?.sessionId == id, isRecording else { return }
        perform(gesture.manualToggle(at: ProcessInfo.processInfo.systemUptime))
    }

    func cancelSession(_ id: String) {
        guard let session = state.workspace.session, session.id == id else { return }
        if session.phase == .working { closeSession(id); return }
        if ownedCapture?.sessionId == id { cancelRecording(); gesture = HotkeyGesture() }
        else if !isRecording { gesture = HotkeyGesture() }
        cancelTargets(id)
        session.phase = .idle
        session.error = nil
    }

    func closeSession(_ id: String) {
        guard state.workspace.session?.id == id else { return }
        if ownedCapture?.sessionId == id { cancelRecording(); gesture = HotkeyGesture() }
        else if !isRecording { gesture = HotkeyGesture() }
        for liveID in previewOwnership.recordings(ownedBy: id) { discardPreview(liveID) }
        cancelTargets(id)
        state.workspace.session = nil
    }

    func cancelExternalPadDelivery() {
        for id in Array(ownedTargets.keys) { cancelTargets(id) }
    }

    func shutdownOwnedSession() {
        state.workspace.flushPad()
        if let session = state.workspace.session { closeSession(session.id) }
    }

    private func cancelTargets(_ id: String) {
        for token in ownedTargets.removeValue(forKey: id) ?? [] { ExternalPasteCoordinator.shared.cancel(token) }
    }

    private func previewTargetName() -> String? {
        guard state.settings.dictationPadAutoPaste, let target = ExternalPasteCoordinator.shared.capture() else { return nil }
        ExternalPasteCoordinator.shared.cancel(target.token)
        return target.name
    }

    var isListening: Bool { monitor?.isRunning ?? false }

    /// Starts the hotkey tap; returns false while Accessibility/Input Monitoring is not granted yet.
    func start() -> Bool {
        if monitor == nil {
            monitor = HotkeyMonitor(
                trigger: state.settings.hotkey,
                onEdge: { [weak self] edge, time in
                    MainActor.assumeIsolated { self?.handle(edge, at: time) }
                },
                onInterrupt: { [weak self] in
                    MainActor.assumeIsolated { self?.interrupt() }
                },
                onShift: { [weak self] down in
                    MainActor.assumeIsolated { self?.shift(down) }
                },
                onAvailabilityChange: { [weak self] available in
                    MainActor.assumeIsolated {
                        guard let self else { return }
                        self.state.globalHotkeyActive = available
                        self.log.notice("sneltoets beschikbaar=\(available) toegankelijkheid=\(Permissions.accessibilityTrusted)")
                    }
                }
            )
        }
        let started = monitor?.start() ?? false
        let settings = state.settings
        log.notice(
            "sneltoets actief=\(started) toegankelijkheid=\(Permissions.accessibilityTrusted) · dicteren \(settings.dictationStt.providerId)/\(settings.dictationStt.model) · opkuis \(settings.llm.providerId)/\(settings.llm.model) · taal \(settings.language ?? "automatisch") · modus \(settings.mode)"
        )
        return started
    }

    func setTrigger(_ trigger: HotkeyTrigger) {
        monitor?.setTrigger(trigger)
        gesture = HotkeyGesture()
    }

    private func handle(_ edge: KeyEdge, at time: TimeInterval) {
        log.debug("hotkey \(String(describing: edge))")
        perform(gesture.handle(edge, at: time))
    }

    private func interrupt() {
        perform(gesture.interrupt())
    }

    private func shift(_ down: Bool) {
        guard ownedCapture == nil else { return }
        guard NSWorkspace.shared.frontmostApplication?.processIdentifier != ProcessInfo.processInfo.processIdentifier else {
            command.reset()
            return
        }
        let wasCommand = command.isCommand
        command.shift(down, keyHeld: isRecording && (monitor?.isKeyDown ?? false))
        if command.isCommand && !wasCommand {
            status.markCommand()
            render()
        }
    }

    private func perform(_ actions: [GestureAction]) {
        for action in actions {
            switch action {
            case .start: beginRecording()
            case .cancel: cancelRecording()
            case .submit: submitRecording()
            }
        }
    }

    private func beginRecording() {
        guard !isRecording else { return }
        guard Permissions.microphone == .authorized else {
            notify("Geen toegang tot de microfoon")
            return
        }
        guard state.dictationSetup.canRecord else {
            gesture = HotkeyGesture()
            notify(state.dictationSetup.reason ?? "Kies eerst een werkende spraakherkenning")
            return
        }
        if ownedCapture == nil,
           NSWorkspace.shared.frontmostApplication?.processIdentifier == ProcessInfo.processInfo.processIdentifier {
            guard let session = state.workspace.session, ownsForeground(session) else {
                gesture = HotkeyGesture()
                command.reset()
                notify("Kies eerst een tekstveld in een andere app")
                return
            }
            startOwnedSession(session.id, manual: false)
            return
        }
        ownedApplicationID = ownedCapture == nil ? nil : Bundle.main.bundleIdentifier
        let live = state.engine.beginDictationSession(
            observer: DictationPreviewForwarder { [weak self] update in
                Task { @MainActor in self?.receivePreview(update) }
            },
            practice: ownedCapture?.kind == .practice
        )
        let liveID = live.id()
        liveSessions[liveID] = live
        previewOwnership.register(liveID, owner: ownedCapture?.sessionId)
        recordingLive = live
        let apple: LiveSpeechPreview?
        if live.mode() == .system {
            apple = LiveSpeechPreview(language: live.language()) { text in live.updateSystemPreview(text: text) }
        } else { apple = nil }
        systemPreview = apple
        let onSamples: @Sendable ([Float]) -> Void
        switch live.mode() {
        case .system: onSamples = { samples in apple?.feed(samples) }
        case .local: onSamples = { samples in live.feedAudio(samples: samples) }
        case .afterRecording: onSamples = { _ in }
        }
        do {
            try recorder.start(onSamples: onSamples) { [weak self] level in
                Task { @MainActor in
                    guard let self, self.isRecording, self.recordingLive?.id() == liveID else { return }
                    self.pill.model.level = level
                    if let id = self.ownedCapture?.sessionId, self.state.workspace.session?.id == id {
                        self.state.workspace.session?.level = level
                    }
                }
            }
        } catch is NoInputDevice {
            discardRecordingPreview()
            notify("Geen microfoon gevonden")
            return
        } catch {
            discardRecordingPreview()
            log.error("opnemen starten mislukt: \(error.localizedDescription)")
            notify("Opnemen lukt niet")
            return
        }
        isRecording = true
        if let id = ownedCapture?.sessionId, state.workspace.session?.id == id { state.workspace.session?.phase = .listening }
        meeting?.setDictating(true)
        status.recordingStarted(handsFree: gesture.isHandsFree)
        command.reset()
        if ownedCapture == nil { command.shift(NSEvent.modifierFlags.contains(.shift), keyHeld: true) }
        if command.isCommand { status.markCommand() }
        render()
        pill.beginPreview(liveID)
        apple?.start()
        ticker = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self else { return }
                self.perform(self.gesture.tick(at: ProcessInfo.processInfo.systemUptime))
            }
        }
    }

    private func ownsForeground(_ session: OwnedDictationSession) -> Bool {
        guard NSWorkspace.shared.frontmostApplication?.processIdentifier == ProcessInfo.processInfo.processIdentifier else { return false }
        switch session.kind {
        case .pad: return NSApp.keyWindow?.identifier?.rawValue == "main" && state.workspace.destination == .dictation
        case .practice: return NSApp.keyWindow?.identifier?.rawValue == "onboarding" && state.settings.onboarding.resumeStep == .practice
        }
    }

    private func receivePreview(_ update: DictationPreviewUpdate) {
        guard previewOwnership.accepts(update.sessionId, currentSession: state.workspace.session?.id) else { return }
        pill.updatePreview(sessionID: update.sessionId, revision: update.revision, text: update.text)
    }

    private func discardRecordingPreview() {
        systemPreview?.stop()
        systemPreview = nil
        if let live = recordingLive { discardPreview(live.id()) }
        recordingLive = nil
    }

    private func discardPreview(_ id: String) {
        liveSessions.removeValue(forKey: id)?.cancel()
        previewOwnership.remove(id)
        pill.clearPreview(sessionID: id)
    }

    private func stopRecording() -> [Float] {
        ticker?.invalidate()
        ticker = nil
        systemPreview?.stop()
        systemPreview = nil
        recordingLive?.stopPreview()
        guard isRecording else { return [] }
        isRecording = false
        meeting?.setDictating(false)
        return recorder.stop()
    }

    private func cancelRecording() {
        _ = stopRecording()
        discardRecordingPreview()
        if let owned = ownedCapture {
            cancelTargets(owned.sessionId)
            if state.workspace.session?.id == owned.sessionId { state.workspace.session?.phase = .idle }
        }
        ownedCapture = nil
        command.reset()
        status.recordingCancelled()
        render()
    }

    private func submitRecording() {
        let owned = ownedCapture
        let live = recordingLive
        let apple = systemPreview
        let external = owned == nil ? ExternalPasteCoordinator.shared.captureCurrentExternal() : nil
        let samples = stopRecording()
        recordingLive = nil
        ownedCapture = nil
        guard !samples.isEmpty else {
            if let live { discardPreview(live.id()) }
            if let external { ExternalPasteCoordinator.shared.cancel(external.token) }
            status.recordingCancelled()
            if let owned, let session = state.workspace.session, session.id == owned.sessionId {
                session.phase = .error
                session.error = "Geen microfoongeluid ontvangen. Probeer het opnieuw."
                cancelTargets(owned.sessionId)
            }
            render()
            return
        }
        if SilenceWatch.isDigitalSilence(samples) {
            if let live { discardPreview(live.id()) }
            if let external { ExternalPasteCoordinator.shared.cancel(external.token) }
            log.error("microfoon leverde enkel stilte (exacte nullen) — invoerapparaat of voice processing")
            command.reset()
            status.recordingCancelled()
            notify("Microfoon geeft geen geluid — kies een ander invoerapparaat of herstart Pennel")
            if let owned, state.workspace.session?.id == owned.sessionId {
                cancelTargets(owned.sessionId)
                state.workspace.session?.phase = .error
                state.workspace.session?.error = "Geen microfoongeluid. Controleer je invoerapparaat en probeer opnieuw."
            }
            return
        }
        guard let live else {
            if let external { ExternalPasteCoordinator.shared.cancel(external.token) }
            status.recordingCancelled()
            notify("Het dictaat kon niet worden afgerond. Probeer opnieuw.")
            return
        }
        let liveID = live.id()
        let seq = queue.reserve()
        pendingWork += 1
        let request = DictationRequest(
            samples: samples,
            sampleRate: UInt32(AudioRecorder.sampleRate),
            appBundleId: DictationApplicationContext.bundleID(
                owned: owned != nil, atStart: ownedApplicationID, atStop: external?.bundleID
            ),
            keys: state.keys()
        )
        let released = ContinuousClock.now
        let isCommand = owned == nil && command.isCommand
        command.reset()
        status.recordingSubmitted()
        if let owned, state.workspace.session?.id == owned.sessionId { state.workspace.session?.phase = .working }
        render()
        if isCommand {
            let pendingPastes = insertion
            Task {
                // Earlier pastes restore the clipboard first, so ⌘C never snapshots Pennel's own text.
                await pendingPastes?.value
                guard let target = external else {
                    self.finish(seq, .failure(CommandError.appChanged), released: released, isCommand: true, liveID: liveID, external: external)
                    return
                }
                let snapshot = await DictationSelectionSnapshot.read(
                    isCurrent: { ExternalPasteCoordinator.shared.isCurrent(target.token) },
                    selection: { await SelectionReader.read() }
                )
                guard case .current(let selection) = snapshot else {
                    self.finish(seq, .failure(CommandError.appChanged), released: released, isCommand: true, liveID: liveID, external: external)
                    return
                }
                let outcome = await Task.detached(priority: .userInitiated) {
                    await apple?.stopAndWait()
                    return Result { try live.finishCommand(request: request, selection: selection) }
                }.value
                let stillThere = ExternalPasteCoordinator.shared.isCurrent(target.token)
                self.finish(seq, stillThere ? outcome : .failure(CommandError.appChanged), released: released, isCommand: true, liveID: liveID, external: external)
            }
            return
        }
        Task.detached(priority: .userInitiated) {
            await apple?.stopAndWait()
            let outcome = Result { try live.finish(request: request) }
            await MainActor.run { self.finish(seq, outcome, released: released, owned: owned, liveID: liveID, external: external) }
        }
    }

    private func finish(
        _ seq: Int,
        _ outcome: Result<DictationResult, Error>,
        released: ContinuousClock.Instant,
        isCommand: Bool = false,
        owned: OwnedCapture? = nil,
        liveID: String,
        external: ExternalPasteSnapshot?
    ) {
        discardPreview(liveID)
        let text: String?
        let notice: String?
        let kind = isCommand ? "opdracht" : "dictaat"
        let elapsed = released.duration(to: .now).formatted(.units(allowed: [.milliseconds]))
        switch outcome {
        case .success(let result) where !result.historySaved:
            if let external { ExternalPasteCoordinator.shared.cancel(external.token) }
            text = nil
            notice = "Bewaren mislukt. Je tekst staat tijdelijk in Geschiedenis."
        case .success(let result):
            text = result.text
            notice = Self.notice(for: result.cleanup)
            let line = "\(kind) klaar in \(elapsed): \(result.raw.count) → \(result.text.count) tekens, opkuis \(Self.describe(result.cleanup))"
            if case .failed = result.cleanup { log.error(line) } else { log.notice(line) }
        case .failure(let error):
            if let external { ExternalPasteCoordinator.shared.cancel(external.token) }
            text = nil
            notice = isCommand ? Self.commandNotice(for: error) : Self.notice(for: error)
            if (error as? PennelError) == .NoSpeech {
                log.notice("\(kind): geen spraak herkend (\(elapsed))")
            } else {
                log.error("\(kind) mislukt na \(elapsed): \(type(of: error))")
            }
        }
        status.dictationFinished(notice: notice)
        if notice != nil { scheduleNoticeExpiry() }
        render()
        NotificationCenter.default.post(name: .pennelHistoryChanged, object: nil)

        pendingWork = max(0, pendingWork - 1)
        if let owned, text == nil, let session = state.workspace.session, session.id == owned.sessionId {
            cancelTargets(owned.sessionId)
            session.phase = .error
            session.error = notice ?? "Geen spraak herkend. Probeer het opnieuw."
        }
        let delivery = text.map { text -> Delivery in
            if let owned { return .owned(text, owned) }
            return .external(text, external)
        }
        let ready = queue.fulfill(seq, with: delivery)
        let previous = insertion
        pendingDeliveries += ready.count
        insertion = Task {
            await previous?.value
            for delivery in ready {
                switch delivery {
                case .external(let text, let target):
                    if let target {
                        do { try await ExternalPasteCoordinator.shared.paste(text: text, token: target.token) }
                        catch ExternalPasteError.cancelled { self.notify("Niet geplakt — het tekstdoel is gewijzigd. Je tekst staat in Geschiedenis.") }
                        catch ExternalPasteError.permission { self.notify("Geen toegang om te plakken. Je tekst staat in Geschiedenis.") }
                        catch { self.notify("Niet geplakt. Je tekst staat in Geschiedenis.") }
                    } else { self.notify("Geen actief tekstdoel — je tekst staat in Geschiedenis.") }
                case .owned(let text, let target): await deliverOwned(text, target: target)
                }
                self.pendingDeliveries = max(0, self.pendingDeliveries - 1)
            }
            if !ready.isEmpty {
                log.info("release→paste \(released.duration(to: .now).formatted(.units(allowed: [.milliseconds])))")
            }
        }
    }

    private func deliverOwned(_ text: String, target: OwnedCapture) async {
        guard let session = state.workspace.session, session.id == target.sessionId else { return }
        if target.kind == .practice {
            session.text = text
            session.phase = .done
            session.error = nil
            state.setOnboarding(step: .practice, status: .completed)
            state.workspace.initialSetup = false
            return
        }
        do {
            state.workspace.padText = try state.engine.appendDictationPad(text: text)
            session.text = state.workspace.padText
            session.phase = .done
            if state.settings.dictationPadAutoPaste {
                if let token = target.externalToken {
                    do { try await ExternalPasteCoordinator.shared.paste(text: text, token: token) }
                    catch { session.error = "Extern plakken lukt niet. Je tekst staat in het dicteerpad." }
                    ownedTargets[target.sessionId]?.remove(token)
                } else {
                    session.error = "Geen doelapp beschikbaar. Je tekst staat in het dicteerpad."
                }
            }
        } catch {
            let previous = state.workspace.padText
            let separator = previous.isEmpty || text.isEmpty || previous.last?.isWhitespace == true || text.first?.isWhitespace == true ? "" : " "
            state.workspace.retainUnsavedPad(previous + separator + text)
            session.text = state.workspace.padText
            session.phase = .error
            session.error = "Dicteerpad bewaren mislukt. Je bewerkte tekst en dictaat staan hier."
            cancelTargets(target.sessionId)
        }
    }

    private func notify(_ message: String) {
        status.showNotice(message)
        scheduleNoticeExpiry()
        render()
    }

    private func scheduleNoticeExpiry() {
        noticeTask?.cancel()
        noticeTask = Task { [weak self] in
            try? await Task.sleep(for: Self.noticeDuration)
            guard !Task.isCancelled, let self else { return }
            self.status.noticeExpired()
            self.render()
        }
    }

    private func render() {
        pill.show(status.pill)
    }

    private static func describe(_ cleanup: CleanupStatus) -> String {
        switch cleanup {
        case .applied: "toegepast"
        case .skipped: "overgeslagen (snelle modus)"
        case .rejected: "afgewezen (model antwoordde in plaats van op te kuisen)"
        case .failed: "mislukt"
        }
    }

    private static func notice(for cleanup: CleanupStatus) -> String? {
        switch cleanup {
        case .failed(let message) where message.lowercased().contains("timed out") || message.lowercased().contains("timeout"):
            "Opkuisen duurde te lang — ruwe tekst geplakt (zie logboek)"
        case .failed: "Opkuisen mislukt — ruwe tekst geplakt (zie logboek)"
        case .rejected: "Model antwoordde i.p.v. op te kuisen — ruwe tekst geplakt"
        default: nil
        }
    }

    private enum CommandError: Error { case appChanged }

    /// Commands are not kept in history when they fail, so these notices don't promise a retry.
    private static func commandNotice(for error: Error) -> String? {
        if case .appChanged = error as? CommandError { return "Andere app actief — opdracht niet geplakt" }
        switch error as? PennelError {
        case .NoSpeech: return nil
        case .MissingApiKey: return "Geen API-sleutel ingesteld"
        case .NotFound: return "Geen taalmodel ingesteld"
        case .Http(let status, _) where status == 401 || status == 403: return "API-sleutel geweigerd"
        case .InvalidResponse(let message) where message == "selection too long": return "Selectie te lang voor een opdracht"
        case .Network: return "Geen verbinding"
        default: return "Opdracht mislukt"
        }
    }

    private static func notice(for error: Error) -> String? {
        switch error as? PennelError {
        case .NoSpeech: nil
        case .MissingApiKey: "Geen API-sleutel ingesteld"
        case .ModelNotDownloaded: "Model nog niet gedownload — bewaard in geschiedenis"
        case .Http(let status, _) where status == 401 || status == 403: "API-sleutel geweigerd — bewaard in geschiedenis"
        case .Network: "Geen verbinding — bewaard in geschiedenis"
        case .Http, .InvalidResponse: "Mislukt — bewaard in geschiedenis"
        default: "Mislukt"
        }
    }
}
