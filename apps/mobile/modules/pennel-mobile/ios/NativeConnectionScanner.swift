@preconcurrency import AVFoundation
import Foundation
import PennelMobileKit
import UIKit

@MainActor
enum NativeConnectionScanner {
    private static var scanning = false

    static func scan(language: String) async throws -> String {
        guard !scanning else { throw PennelBridgeError("operation_busy", "A code scanner is already open.") }
        scanning = true
        defer { scanning = false }
        let allowed: Bool
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized: allowed = true
        case .notDetermined: allowed = await AVCaptureDevice.requestAccess(for: .video)
        default: allowed = false
        }
        guard allowed else { throw PennelBridgeError("camera_denied", "Allow camera access to scan a connection code, or enter the code instead.") }
        guard let scene = UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }).first(where: { $0.activationState == .foregroundActive }),
              let root = scene.windows.first(where: \.isKeyWindow)?.rootViewController else {
            throw PennelBridgeError("scanner_unavailable", "Open Pennel to scan a connection code.")
        }
        var presenter = root
        while let presented = presenter.presentedViewController { presenter = presented }
        let scanner = ConnectionScannerController(language: language)
        try await scanner.prepare()
        let navigation = UINavigationController(rootViewController: scanner)
        navigation.modalPresentationStyle = .fullScreen
        return try await withCheckedThrowingContinuation { continuation in
            scanner.finished = { result in
                navigation.dismiss(animated: true) { continuation.resume(with: result) }
            }
            presenter.present(navigation, animated: true)
        }
    }
}

@MainActor
private final class ConnectionScannerController: UIViewController {
    var finished: ((Result<String, Error>) -> Void)?
    private let language: String
    private let capture = ConnectionScannerCapture()
    private var preview: AVCaptureVideoPreviewLayer?
    private var didFinish = false
    private lazy var reader = ConnectionCodeReader { [weak self] code in self?.finish(.success(code)) }

    init(language: String) {
        self.language = language
        super.init(nibName: nil, bundle: nil)
    }

    required init?(coder: NSCoder) { nil }

    func prepare() async throws { try await capture.prepare(reader: reader) }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        title = language == "nl" ? "Verbindingscode scannen" : "Scan connection code"
        navigationItem.leftBarButtonItem = UIBarButtonItem(title: language == "nl" ? "Annuleer" : "Cancel", style: .plain, target: self, action: #selector(cancel))
        let preview = AVCaptureVideoPreviewLayer(session: capture.session)
        preview.videoGravity = .resizeAspectFill
        view.layer.addSublayer(preview)
        self.preview = preview
        let help = UILabel()
        help.text = language == "nl" ? "Richt de camera op de QR-code in Pennel op je computer." : "Point your camera at the QR code in Pennel on your computer."
        help.font = UIFont.preferredFont(forTextStyle: .body)
        help.adjustsFontForContentSizeCategory = true
        help.numberOfLines = 0
        help.textAlignment = .center
        help.textColor = .label
        help.backgroundColor = .systemBackground
        help.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(help)
        NSLayoutConstraint.activate([
            help.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: Brand.Spacing.s5),
            help.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -Brand.Spacing.s5),
            help.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor, constant: -Brand.Spacing.s5),
        ])
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        preview?.frame = view.safeAreaLayoutGuide.layoutFrame
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        capture.start()
    }

    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        capture.stop()
    }

    @objc private func cancel() { finish(.failure(PennelBridgeError("cancelled", "Connection scanning cancelled."))) }

    private func finish(_ result: Result<String, Error>) {
        guard !didFinish else { return }
        didFinish = true
        capture.stop()
        finished?(result)
        finished = nil
    }
}

private final class ConnectionCodeReader: NSObject, AVCaptureMetadataOutputObjectsDelegate, @unchecked Sendable {
    private let found: @MainActor @Sendable (String) -> Void
    init(found: @escaping @MainActor @Sendable (String) -> Void) { self.found = found }

    func metadataOutput(_ output: AVCaptureMetadataOutput, didOutput metadataObjects: [AVMetadataObject], from connection: AVCaptureConnection) {
        guard let item = metadataObjects.compactMap({ $0 as? AVMetadataMachineReadableCodeObject }).first(where: { $0.type == .qr }),
              let code = item.stringValue, !code.isEmpty, code.utf8.count <= 4_096 else { return }
        Task { @MainActor in found(code) }
    }
}

private final class ConnectionScannerCapture: @unchecked Sendable {
    let session = AVCaptureSession()
    private let queue = DispatchQueue(label: "app.pennel.connection-scanner")

    func prepare(reader: ConnectionCodeReader) async throws {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            queue.async {
                do {
                    guard let camera = AVCaptureDevice.default(for: .video) else { throw PennelBridgeError("scanner_unavailable", "This device has no available camera. Enter the connection code instead.") }
                    let input = try AVCaptureDeviceInput(device: camera)
                    let output = AVCaptureMetadataOutput()
                    self.session.beginConfiguration()
                    defer { self.session.commitConfiguration() }
                    self.session.automaticallyConfiguresApplicationAudioSession = false
                    guard self.session.canAddInput(input), self.session.canAddOutput(output) else { throw PennelBridgeError("scanner_unavailable", "The camera is unavailable. Enter the connection code instead.") }
                    self.session.addInput(input)
                    self.session.addOutput(output)
                    output.setMetadataObjectsDelegate(reader, queue: .main)
                    output.metadataObjectTypes = [.qr]
                    continuation.resume()
                } catch { continuation.resume(throwing: error) }
            }
        }
    }

    func start() { queue.async { self.session.startRunning() } }
    func stop() { queue.async { self.session.stopRunning() } }
}
