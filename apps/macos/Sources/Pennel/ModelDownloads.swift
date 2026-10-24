import Foundation
@preconcurrency import PennelCore
import Observation

private final class ProgressRelay: DownloadListener, @unchecked Sendable {
    private let onUpdate: @Sendable (Double) -> Void
    private var lastReported = -1.0

    init(onUpdate: @escaping @Sendable (Double) -> Void) {
        self.onUpdate = onUpdate
    }

    func onProgress(downloaded: UInt64, total: UInt64) {
        guard total > 0 else { return }
        let fraction = Double(downloaded) / Double(total)
        guard fraction - lastReported >= 0.005 || fraction >= 1 else { return }
        lastReported = fraction
        onUpdate(fraction)
    }
}

@MainActor
@Observable
final class ModelDownloads {
    /// Downloaded legacy files included; the screen shows `visible`, pickers use `models`.
    private(set) var models: [LocalModel] = []
    private(set) var visible: [LocalModel] = []
    var scoreLanguage = "nl" { didSet { reload() } }
    var supportedOnly = true { didSet { reload() } }
    private(set) var states: [String: ModelState] = [:]
    private(set) var progress: [String: Double] = [:]
    private(set) var errors: [String: String] = [:]
    private let engine: PennelEngine
    private let queue = DispatchQueue(label: "dev.pennel.downloads", qos: .utility, attributes: .concurrent)

    init(engine: PennelEngine) {
        self.engine = engine
        reload()
    }

    var downloaded: [LocalModel] {
        models.filter { states[$0.id] == .downloaded }
    }

    func isDownloading(_ id: String) -> Bool {
        progress[id] != nil
    }

    func reload() {
        models = engine.localModels(language: scoreLanguage, supportedOnly: false)
        visible = engine.localModels(language: scoreLanguage, supportedOnly: supportedOnly)
        for model in models {
            states[model.id] = engine.modelState(id: model.id)
        }
    }

    func refresh() {
        reload()
    }

    /// `sizeBytes` is needed for models outside the speech list (the diarizer) to resume the bar at the right spot.
    func start(_ id: String, sizeBytes: UInt64? = nil) {
        guard progress[id] == nil else { return }
        errors[id] = nil
        progress[id] = partialFraction(id, size: sizeBytes)
        let engine = engine
        let relay = ProgressRelay { [weak self] fraction in
            Task { @MainActor in
                if self?.progress[id] != nil { self?.progress[id] = fraction }
            }
        }
        queue.async { [weak self] in
            let result = Result { try engine.downloadModel(id: id, listener: relay) }
            Task { @MainActor in self?.finish(id, result) }
        }
    }

    func cancel(_ id: String) {
        engine.cancelDownload(id: id)
    }

    var onModelsChanged: (() -> Void)?

    func delete(_ id: String) {
        try? engine.deleteModel(id: id)
        refresh()
        onModelsChanged?()
    }

    private func finish(_ id: String, _ result: Result<Void, Error>) {
        progress[id] = nil
        switch result {
        case .success:
            errors[id] = nil
            onModelsChanged?()
        case .failure(let error):
            if case .Cancelled = error as? PennelError {
                errors[id] = nil
            } else {
                errors[id] = "Download mislukt: \(error)"
                Diagnostics("models").error("download \(id) mislukt: \(error)")
            }
        }
        refresh()
    }

    private func partialFraction(_ id: String, size known: UInt64?) -> Double {
        guard case .partial(let bytes) = engine.modelState(id: id),
              let size = known ?? models.first(where: { $0.id == id })?.sizeBytes, size > 0
        else { return 0 }
        return Double(bytes) / Double(size)
    }
}
