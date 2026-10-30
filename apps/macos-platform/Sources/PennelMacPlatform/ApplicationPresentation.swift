import AppKit

func productWindowActivationPolicy(visible: Bool) -> NSApplication.ActivationPolicy {
    visible ? .regular : .accessory
}

@_cdecl("pennel_mac_product_windows_visible")
public func productWindowsVisible(_ visible: Bool) {
    onMain {
        let application = NSApplication.shared
        let policy = productWindowActivationPolicy(visible: visible)
        if application.activationPolicy() != policy {
            _ = application.setActivationPolicy(policy)
        }
    }
}
