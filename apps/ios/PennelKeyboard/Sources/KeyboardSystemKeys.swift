import PennelMobileKit
import SwiftUI
import UIKit

@MainActor
final class KeyboardInputModeTarget {
    weak var controller: UIInputViewController?
}

struct KeyboardGlobeKey: UIViewRepresentable {
    let target: KeyboardInputModeTarget
    let label: String

    func makeUIView(context: Context) -> UIButton {
        let button = UIButton(type: .system)
        button.setImage(UIImage(systemName: "globe"), for: .normal)
        button.addTarget(target.controller, action: #selector(UIInputViewController.handleInputModeList(from:with:)), for: .allTouchEvents)
        return button
    }

    func updateUIView(_ button: UIButton, context: Context) {
        button.accessibilityLabel = label
        button.tintColor = UIColor(Brand.foreground.color)
        button.backgroundColor = UIColor(Brand.subtle.color)
        button.layer.cornerRadius = Brand.Radius.keycap
    }
}

struct KeyboardDeleteKey: UIViewRepresentable {
    let label: String
    let action: () -> Void

    func makeUIView(context: Context) -> RepeatingDeleteButton { RepeatingDeleteButton() }

    func updateUIView(_ button: RepeatingDeleteButton, context: Context) {
        button.action = action
        button.accessibilityLabel = label
        button.tintColor = UIColor(Brand.foreground.color)
        button.backgroundColor = UIColor(Brand.subtle.color)
        button.layer.cornerRadius = Brand.Radius.keycap
    }

    static func dismantleUIView(_ button: RepeatingDeleteButton, coordinator: ()) { button.stopRepeating() }
}

final class RepeatingDeleteButton: UIButton {
    var action: (() -> Void)?
    private var repeatTimer: Timer?

    override init(frame: CGRect) {
        super.init(frame: frame)
        setImage(UIImage(systemName: "delete.left"), for: .normal)
        addTarget(self, action: #selector(pressed), for: .touchDown)
        addTarget(self, action: #selector(released), for: [.touchUpInside, .touchUpOutside, .touchCancel, .touchDragExit])
    }

    required init?(coder: NSCoder) { nil }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        if window == nil { stopRepeating() }
    }

    override func accessibilityActivate() -> Bool {
        action?()
        return true
    }

    @objc private func pressed() {
        stopRepeating()
        action?()
        repeatTimer = Timer.scheduledTimer(timeInterval: 0.38, target: self, selector: #selector(startRepeating), userInfo: nil, repeats: false)
    }

    @objc private func startRepeating() {
        action?()
        repeatTimer = Timer.scheduledTimer(timeInterval: 0.075, target: self, selector: #selector(repeatDelete), userInfo: nil, repeats: true)
    }

    @objc private func repeatDelete() { action?() }
    @objc private func released() { stopRepeating() }

    func stopRepeating() {
        repeatTimer?.invalidate()
        repeatTimer = nil
    }
}
