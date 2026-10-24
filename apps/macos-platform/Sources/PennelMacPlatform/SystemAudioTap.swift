@preconcurrency import AVFoundation
import CoreAudio

struct SystemAudioTapError: Error {
    let step: String
    let status: OSStatus
}

/// Captures everything the Mac plays (except Pennel itself) via a Core Audio process tap,
/// which only needs the "System Audio Recording Only" permission, not Screen Recording.
final class SystemAudioTap: @unchecked Sendable {
    private var tapID = AudioObjectID(kAudioObjectUnknown)
    private var aggregateID = AudioObjectID(kAudioObjectUnknown)
    private var procID: AudioDeviceIOProcID?
    private let queue = DispatchQueue(label: "dev.pennel.systemaudio", qos: .userInitiated)

    private var description: CATapDescription?
    private var tapOnly: UnsafeMutableAudioBufferListPointer?
    private var format: AVAudioFormat?

    /// Creates the tap and returns the format its buffers will have.
    /// `processes` limits it to one app's audio (a detected call); nil taps everything except Pennel.
    func prepare(processes: [AudioObjectID]? = nil) throws -> AVAudioFormat {
        let description = if let processes, !processes.isEmpty {
            CATapDescription(stereoMixdownOfProcesses: processes)
        } else {
            CATapDescription(stereoGlobalTapButExcludeProcesses: ownProcessObject().map { [$0] } ?? [])
        }
        description.uuid = UUID()
        description.muteBehavior = .unmuted
        description.isPrivate = true
        try check("create tap", AudioHardwareCreateProcessTap(description, &tapID))

        var streamDescription = try tapFormat()
        guard let format = AVAudioFormat(streamDescription: &streamDescription) else {
            throw SystemAudioTapError(step: "tap format", status: -1)
        }
        self.description = description
        self.format = format
        return format
    }

    func start(onBuffer: @escaping @Sendable (AVAudioPCMBuffer) -> Void) throws {
        guard let description, let format else { throw SystemAudioTapError(step: "not prepared", status: -1) }
        let outputUID = try defaultOutputUID()
        let aggregate: [String: Any] = [
            kAudioAggregateDeviceNameKey: "Pennel System Audio",
            kAudioAggregateDeviceUIDKey: UUID().uuidString,
            kAudioAggregateDeviceMainSubDeviceKey: outputUID,
            kAudioAggregateDeviceIsPrivateKey: true,
            kAudioAggregateDeviceIsStackedKey: false,
            kAudioAggregateDeviceTapAutoStartKey: true,
            kAudioAggregateDeviceSubDeviceListKey: [[kAudioSubDeviceUIDKey: outputUID]],
            kAudioAggregateDeviceTapListKey: [[
                kAudioSubTapDriftCompensationKey: true,
                kAudioSubTapUIDKey: description.uuid.uuidString,
            ]],
        ]
        try check("create aggregate device", AudioHardwareCreateAggregateDevice(aggregate as CFDictionary, &aggregateID))

        let wanted = format.isInterleaved ? 1 : Int(format.channelCount)
        let tapOnly = AudioBufferList.allocate(maximumBuffers: wanted)
        self.tapOnly = tapOnly
        try check("create io proc", AudioDeviceCreateIOProcIDWithBlock(&procID, aggregateID, queue) { _, input, _, _, _ in
            let all = UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: input))
            guard let range = TapBuffers.range(total: all.count, interleaved: format.isInterleaved, channels: Int(format.channelCount))
            else { return }
            for (slot, index) in range.enumerated() { tapOnly[slot] = all[index] }
            guard let buffer = AVAudioPCMBuffer(pcmFormat: format, bufferListNoCopy: tapOnly.unsafePointer, deallocator: nil)
            else { return }
            onBuffer(buffer)
        })
        try check("start device", AudioDeviceStart(aggregateID, procID))
    }

    func stop() {
        if aggregateID != kAudioObjectUnknown {
            if let procID {
                AudioDeviceStop(aggregateID, procID)
                AudioDeviceDestroyIOProcID(aggregateID, procID)
            }
            AudioHardwareDestroyAggregateDevice(aggregateID)
        }
        if tapID != kAudioObjectUnknown {
            AudioHardwareDestroyProcessTap(tapID)
        }
        procID = nil
        aggregateID = AudioObjectID(kAudioObjectUnknown)
        tapID = AudioObjectID(kAudioObjectUnknown)
        tapOnly?.unsafeMutablePointer.deallocate()
        tapOnly = nil
        description = nil
        format = nil
    }

    deinit { stop() }

    /// Calls `handler` on the main queue when the default output device changes (headphones in/out).
    static func observeOutputChanges(_ handler: @escaping @Sendable () -> Void) -> AudioObjectPropertyListenerBlock {
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDefaultOutputDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        let block: AudioObjectPropertyListenerBlock = { _, _ in handler() }
        AudioObjectAddPropertyListenerBlock(AudioObjectID(kAudioObjectSystemObject), &address, .main, block)
        return block
    }

    static func stopObserving(_ block: @escaping AudioObjectPropertyListenerBlock) {
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDefaultOutputDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        AudioObjectRemovePropertyListenerBlock(AudioObjectID(kAudioObjectSystemObject), &address, .main, block)
    }

    private func check(_ step: String, _ status: OSStatus) throws {
        guard status == noErr else { throw SystemAudioTapError(step: step, status: status) }
    }

    private func tapFormat() throws -> AudioStreamBasicDescription {
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioTapPropertyFormat,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        var format = AudioStreamBasicDescription()
        var size = UInt32(MemoryLayout<AudioStreamBasicDescription>.size)
        try check("read tap format", AudioObjectGetPropertyData(tapID, &address, 0, nil, &size, &format))
        return format
    }

    private func defaultOutputUID() throws -> String {
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDefaultOutputDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        var device = AudioObjectID(kAudioObjectUnknown)
        var size = UInt32(MemoryLayout<AudioObjectID>.size)
        try check(
            "default output",
            AudioObjectGetPropertyData(AudioObjectID(kAudioObjectSystemObject), &address, 0, nil, &size, &device)
        )
        address.mSelector = kAudioDevicePropertyDeviceUID
        var uid: Unmanaged<CFString>?
        size = UInt32(MemoryLayout<Unmanaged<CFString>?>.size)
        try check("output uid", AudioObjectGetPropertyData(device, &address, 0, nil, &size, &uid))
        guard let uid else { throw SystemAudioTapError(step: "output uid", status: -1) }
        return uid.takeRetainedValue() as String
    }

    private func ownProcessObject() -> AudioObjectID? {
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyTranslatePIDToProcessObject,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        var pid = ProcessInfo.processInfo.processIdentifier
        var object = AudioObjectID(kAudioObjectUnknown)
        var size = UInt32(MemoryLayout<AudioObjectID>.size)
        let status = AudioObjectGetPropertyData(
            AudioObjectID(kAudioObjectSystemObject), &address,
            UInt32(MemoryLayout<pid_t>.size), &pid, &size, &object
        )
        return status == noErr && object != kAudioObjectUnknown ? object : nil
    }
}
