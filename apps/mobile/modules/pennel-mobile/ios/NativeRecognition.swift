import AVFoundation
import CryptoKit
import Foundation

final class NativeRecognitionReader: RecognitionAudioReader, @unchecked Sendable {
    private let archive: URL
    private let lock = NSLock()
    private var file: AVAudioFile?
    private var snapshotDirectory: URL?
    private var snapshotDigest: String?
    private static let snapshotPrefix = "pennel-recognition-reader-"

    init(archive: URL) { self.archive = archive }

    func info() throws -> RecognitionAudioInfo {
        try lock.withLock {
            let handle = try open()
            let digest = try Self.digest(archive)
            guard digest == snapshotDigest else { throw PennelError.Cloud(code: "speaker_stale", message: "The recording changed during speaker recognition.") }
            return RecognitionAudioInfo(totalFrames: UInt64(handle.length), audioDigest: digest)
        }
    }

    func readFrames(offset: UInt64, maxFrames: UInt32) throws -> RecognitionAudioBlock {
        try lock.withLock {
            guard maxFrames > 0, maxFrames <= 64_000, offset <= UInt64(Int64.max) else {
                throw PennelError.InvalidResponse(message: "Invalid bounded audio read.")
            }
            let source = try open()
            guard offset <= UInt64(source.length) else { throw PennelError.InvalidResponse(message: "Audio offset is outside the recording.") }
            let remaining = UInt64(source.length) - offset
            guard remaining > 0 else { return RecognitionAudioBlock(samples: [], eof: true) }
            let frameCount = AVAudioFrameCount(min(UInt64(maxFrames), remaining))
            source.framePosition = AVAudioFramePosition(offset)
            guard let buffer = AVAudioPCMBuffer(pcmFormat: source.processingFormat, frameCapacity: frameCount) else {
                throw PennelError.Storage(message: "Could not allocate the bounded audio buffer.")
            }
            do { try source.read(into: buffer, frameCount: frameCount) }
            catch { throw PennelError.Storage(message: "The local recording could not be decoded.") }
            guard let channel = buffer.floatChannelData?.pointee else {
                throw PennelError.InvalidResponse(message: "The archive could not be decoded as audio.")
            }
            let samples = Array(UnsafeBufferPointer(start: channel, count: Int(buffer.frameLength)))
            return RecognitionAudioBlock(samples: samples, eof: source.framePosition >= source.length)
        }
    }

    private func open() throws -> AVAudioFile {
        if let file { return file }
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(Self.snapshotPrefix + UUID().uuidString, isDirectory: true)
        let snapshot = directory.appendingPathComponent("archive").appendingPathExtension(archive.pathExtension)
        let opened: AVAudioFile
        do {
            let values = try archive.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey])
            guard values.isRegularFile == true, values.isSymbolicLink != true else {
                throw PennelError.Storage(message: "Use a regular local recording file.")
            }
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
            try FileManager.default.copyItem(at: archive, to: snapshot)
            try FileManager.default.setAttributes([.posixPermissions: 0o600, .protectionKey: FileProtectionType.completeUntilFirstUserAuthentication], ofItemAtPath: snapshot.path)
            let digest = try Self.digest(snapshot)
            guard digest == (try Self.digest(archive)) else { throw PennelError.Cloud(code: "speaker_stale", message: "The recording changed before speaker recognition.") }
            opened = try AVAudioFile(forReading: snapshot, commonFormat: .pcmFormatFloat32, interleaved: false)
            snapshotDigest = digest
        } catch let error as PennelError {
            try? FileManager.default.removeItem(at: directory)
            throw error
        } catch {
            try? FileManager.default.removeItem(at: directory)
            throw PennelError.Storage(message: "The local recording is missing, unreadable, or there is not enough temporary space.")
        }
        guard opened.processingFormat.sampleRate == 16_000, opened.processingFormat.channelCount == 1 else {
            try? FileManager.default.removeItem(at: directory)
            throw PennelError.InvalidResponse(message: "This recording is not a supported 16 kHz mono archive.")
        }
        snapshotDirectory = directory
        file = opened
        return opened
    }

    private static func digest(_ url: URL) throws -> String {
        do {
            var hash = SHA256()
            let source = try FileHandle(forReadingFrom: url)
            defer { try? source.close() }
            while let chunk = try source.read(upToCount: 64 * 1024), !chunk.isEmpty { hash.update(data: chunk) }
            return hash.finalize().map { String(format: "%02x", $0) }.joined()
        } catch { throw PennelError.Storage(message: "The local recording is missing or unreadable.") }
    }

    static func discardStaleSnapshots() {
        let directories = (try? FileManager.default.contentsOfDirectory(at: FileManager.default.temporaryDirectory, includingPropertiesForKeys: [.isDirectoryKey, .isSymbolicLinkKey])) ?? []
        for directory in directories where directory.lastPathComponent.hasPrefix(snapshotPrefix) {
            let values = try? directory.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey])
            if values?.isDirectory == true && values?.isSymbolicLink != true { try? FileManager.default.removeItem(at: directory) }
        }
    }

    deinit {
        file = nil
        if let snapshotDirectory { try? FileManager.default.removeItem(at: snapshotDirectory) }
    }
}

final class NativeRecognitionTransport: RecognitionFileTransport, @unchecked Sendable {
    private let lock = NSLock()
    private var active: [String: NativeRecognitionUpload] = [:]
    private var cancelled: Set<String> = []

    func upload(request: RecognitionUploadRequest, progress: RecognitionUploadProgress) throws -> RecognitionUploadResponse {
        let upload = NativeRecognitionUpload(request: request, progress: progress)
        let shouldCancel = lock.withLock {
            active[request.requestId] = upload
            return cancelled.contains(request.requestId)
        }
        defer { lock.withLock { active.removeValue(forKey: request.requestId); cancelled.remove(request.requestId) } }
        if shouldCancel { throw PennelError.Cancelled }
        return try upload.run()
    }

    func cancel(requestId: String) {
        let upload = lock.withLock {
            cancelled.insert(requestId)
            return active[requestId]
        }
        upload?.cancel()
    }
}

private final class NativeRecognitionUpload: NSObject, URLSessionDataDelegate, URLSessionTaskDelegate, @unchecked Sendable {
    private let request: RecognitionUploadRequest
    private let progress: RecognitionUploadProgress
    private let lock = NSLock()
    private let finished = DispatchSemaphore(value: 0)
    private var session: URLSession?
    private var task: URLSessionUploadTask?
    private var response: HTTPURLResponse?
    private var body = Data()
    private var result: Result<RecognitionUploadResponse, Error>?
    private var cancelled = false
    private static let maximumResponseBytes = 4 * 1024 * 1024

    init(request: RecognitionUploadRequest, progress: RecognitionUploadProgress) {
        self.request = request
        self.progress = progress
    }

    func run() throws -> RecognitionUploadResponse {
        guard let url = URL(string: request.url), url.scheme == "https" || url.host == "127.0.0.1" || url.host == "localhost" else {
            throw PennelError.InvalidResponse(message: "Cloud recognition requires a secure endpoint.")
        }
        var uploadRequest = URLRequest(url: url)
        uploadRequest.httpMethod = "POST"
        uploadRequest.timeoutInterval = TimeInterval(request.timeoutSeconds)
        uploadRequest.setValue("Bearer " + request.bearerToken, forHTTPHeaderField: "Authorization")
        uploadRequest.setValue("audio/wav", forHTTPHeaderField: "Content-Type")
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = TimeInterval(request.timeoutSeconds)
        configuration.timeoutIntervalForResource = TimeInterval(request.timeoutSeconds)
        configuration.urlCache = nil
        configuration.httpCookieStorage = nil
        configuration.urlCredentialStorage = nil
        configuration.httpShouldSetCookies = false
        configuration.requestCachePolicy = .reloadIgnoringLocalCacheData
        let queue = OperationQueue()
        queue.maxConcurrentOperationCount = 1
        let session = URLSession(configuration: configuration, delegate: self, delegateQueue: queue)
        let task = session.uploadTask(with: uploadRequest, fromFile: URL(fileURLWithPath: request.filePath))
        let started = lock.withLock {
            guard !cancelled else { return false }
            self.session = session
            self.task = task
            task.resume()
            return true
        }
        guard started else {
            session.invalidateAndCancel()
            throw PennelError.Cancelled
        }
        finished.wait()
        let outcome = lock.withLock { result }
        session.finishTasksAndInvalidate()
        guard let outcome else { throw PennelError.Network(message: "Cloud recognition ended without a response.") }
        return try outcome.get()
    }

    func cancel() {
        let currentTask = lock.withLock { cancelled = true; return self.task }
        currentTask?.cancel()
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didSendBodyData bytesSent: Int64, totalBytesSent: Int64, totalBytesExpectedToSend: Int64) {
        progress.sent(sentBytes: UInt64(max(0, totalBytesSent)), totalBytes: UInt64(max(0, totalBytesExpectedToSend)))
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive response: URLResponse, completionHandler: @escaping @Sendable (URLSession.ResponseDisposition) -> Void) {
        self.response = response as? HTTPURLResponse
        completionHandler(response.expectedContentLength > Int64(Self.maximumResponseBytes) ? .cancel : .allow)
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        guard body.count <= Self.maximumResponseBytes - data.count else { dataTask.cancel(); return }
        body.append(data)
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest, completionHandler: @escaping @Sendable (URLRequest?) -> Void) {
        completionHandler(nil)
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        let result: Result<RecognitionUploadResponse, Error>
        if let error {
            result = .failure(PennelError.Network(message: (error as NSError).code == NSURLErrorCancelled ? "Recognition upload was interrupted." : "Cloud recognition did not return a complete response."))
        } else if let response, let text = String(data: body, encoding: .utf8), (100...599).contains(response.statusCode) {
            result = .success(RecognitionUploadResponse(status: UInt16(response.statusCode), body: text))
        } else {
            result = .failure(PennelError.InvalidResponse(message: "Cloud recognition returned an unreadable response."))
        }
        lock.withLock { self.result = result; self.task = nil; self.session = nil }
        finished.signal()
    }
}
