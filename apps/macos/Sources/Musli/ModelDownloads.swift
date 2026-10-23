import Foundation
@preconcurrency import MusliCore
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
    let models = localModels()
    private(set) var states: [String: ModelState] = [:]
    private(set) var progress: [String: Double] = [:]
    private(set) var errors: [String: String] = [:]
    private let engine: MusliEngine
    private let queue = DispatchQueue(label: "dev.musli.downloads", qos: .utility, attributes: .concurrent)

    init(engine: MusliEngine) {
        self.engine = engine
        refresh()
    }

    var downloaded: [LocalModel] {
        models.filter { states[$0.id] == .downloaded }
    }

    func isDownloading(_ id: String) -> Bool {
        progress[id] != nil
    }

    func refresh() {
        for model in models {
            states[model.id] = engine.modelState(id: model.id)
        }
    }

    func start(_ id: String) {
        guard progress[id] == nil else { return }
        errors[id] = nil
        progress[id] = partialFraction(id)
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
            if case .Cancelled = error as? MusliError {
                errors[id] = nil
            } else {
                errors[id] = "Download mislukt: \(error)"
            }
        }
        refresh()
    }

    private func partialFraction(_ id: String) -> Double {
        guard case .partial(let bytes) = engine.modelState(id: id),
              let size = models.first(where: { $0.id == id })?.sizeBytes, size > 0
        else { return 0 }
        return Double(bytes) / Double(size)
    }
}
