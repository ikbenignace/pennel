import Foundation

public enum AudioInput {
    /// A Mac without a microphone reports a 0 Hz / 0 channel input format, which AVAudioEngine cannot tap.
    public static func isUsable(sampleRate: Double, channels: UInt32) -> Bool {
        sampleRate > 0 && channels > 0
    }
}
