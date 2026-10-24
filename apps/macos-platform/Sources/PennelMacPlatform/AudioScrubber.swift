import AVFoundation

final class AudioScrubber {
    static let shared = AudioScrubber()
    private var player: AVAudioPlayer?

    func play(_ url: URL, fromMs ms: UInt64) {
        player?.stop()
        guard let player = try? AVAudioPlayer(contentsOf: url) else { return }
        player.currentTime = Double(ms) / 1000
        player.play()
        self.player = player
    }

    func stop() {
        player?.stop()
    }
}
