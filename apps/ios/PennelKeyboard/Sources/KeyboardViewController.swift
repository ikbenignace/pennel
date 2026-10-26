import PennelMobileKit
import SwiftUI
import UIKit

final class KeyboardViewController: UIInputViewController {
    private var host: UIHostingController<KeyboardRoot>?
    private var model: KeyboardModel?
    private var timer: Timer?
    private var observations: [HandoffSignal.Observation] = []

    override func viewDidLoad() {
        super.viewDidLoad()
        let height = view.heightAnchor.constraint(equalToConstant: 260)
        height.priority = .defaultHigh
        height.isActive = true
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        install()
        model?.refresh()
        // Darwin signals are the fast path; the timer also catches a heartbeat that simply stops (5 s) and animates the bars.
        observations = [HandoffSignal.session, .result].map { signal in
            signal.observe { [weak self] in DispatchQueue.main.async { self?.model?.refresh() } }
        }
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: 0.2, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.model?.refresh() }
        }
    }

    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        timer?.invalidate()
        timer = nil
        observations = []
    }

    private func install() {
        let store = HandoffStore(directory: HandoffStore.groupDirectory ?? FileManager.default.temporaryDirectory.appendingPathComponent("pennel-no-group"))
        let model = KeyboardModel(
            hasFullAccess: hasFullAccess,
            client: HandoffClient(store: store),
            openApp: { [weak self] in
                guard let self else { return false }
                return AppOpener.openSession(from: view)
            },
            onResult: { [weak self] in self?.textDocumentProxy.insertText($0) })
        self.model = model
        // Without Full Access the shared container is off limits; the default layout applies.
        let layout = hasFullAccess ? KeyboardLayout.stored(in: KeyboardLayout.groupDefaults) : .qwerty
        if hasFullAccess { KeyboardFullAccess.markSeen(in: KeyboardLayout.groupDefaults) }
        let root = KeyboardRoot(
            model: model, layout: layout, showsGlobe: needsInputModeSwitchKey,
            advance: { [weak self] in self?.advanceToNextInputMode() },
            openSettings: { [weak self] in
                guard let self else { return }
                _ = AppOpener.openSettings(from: view)
            },
            insert: { [weak self] in self?.textDocumentProxy.insertText($0) },
            delete: { [weak self] in self?.textDocumentProxy.deleteBackward() })
        if let host {
            host.rootView = root
            return
        }
        let host = UIHostingController(rootView: root)
        addChild(host)
        host.view.translatesAutoresizingMaskIntoConstraints = false
        host.view.backgroundColor = .clear
        view.addSubview(host.view)
        NSLayoutConstraint.activate([
            host.view.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            host.view.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            host.view.topAnchor.constraint(equalTo: view.topAnchor),
            host.view.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
        host.didMove(toParent: self)
        self.host = host
    }
}
