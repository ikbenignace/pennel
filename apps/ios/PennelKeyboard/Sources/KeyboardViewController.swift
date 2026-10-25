import UIKit

/// Placeholder until the dictation keyboard lands; it only offers the system's next-keyboard key.
final class KeyboardViewController: UIInputViewController {
    override func viewDidLoad() {
        super.viewDidLoad()
        let next = UIButton(type: .system, primaryAction: UIAction { [weak self] _ in self?.advanceToNextInputMode() })
        next.setImage(UIImage(systemName: "globe"), for: .normal)
        next.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(next)
        NSLayoutConstraint.activate([
            next.leadingAnchor.constraint(equalTo: view.layoutMarginsGuide.leadingAnchor),
            next.bottomAnchor.constraint(equalTo: view.layoutMarginsGuide.bottomAnchor),
            next.widthAnchor.constraint(greaterThanOrEqualToConstant: 44),
            next.heightAnchor.constraint(greaterThanOrEqualToConstant: 44),
        ])
    }
}
