import AppKit
import Foundation
import WebKit

func processCPUSeconds() -> Double {
    var usage = rusage()
    getrusage(RUSAGE_SELF, &usage)
    return Double(usage.ru_utime.tv_sec + usage.ru_stime.tv_sec) + Double(usage.ru_utime.tv_usec + usage.ru_stime.tv_usec) / 1_000_000
}

@MainActor
final class Runner: NSObject, NSApplicationDelegate, WKNavigationDelegate, WKScriptMessageHandler {
    let url: URL
    let output: URL
    var window: NSWindow!
    var webview: WKWebView!
    var finished = false
    let started = ProcessInfo.processInfo.systemUptime
    let startedCPU = processCPUSeconds()
    var idleStart: (wall: Double, cpu: Double)?
    var idleResourceUsage: [String: Any] = [:]

    init(url: URL, output: URL) {
        self.url = url
        self.output = output
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        configuration.userContentController.add(self, name: "benchmark")
        webview = WKWebView(frame: NSRect(x: 0, y: 0, width: 980, height: 640), configuration: configuration)
        webview.navigationDelegate = self
        window = NSWindow(contentRect: webview.frame, styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false)
        window.title = "Pennel — synthetic WebKit performance check"
        window.contentView = webview
        window.center()
        window.orderFront(nil)
        webview.load(URLRequest(url: url))
        DispatchQueue.main.asyncAfter(deadline: .now() + 90) { [weak self] in
            self?.complete(["error": "The owned WKWebView benchmark timed out."])
        }
    }

    func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        let destination = navigationAction.request.url
        decisionHandler(destination?.host == "127.0.0.1" && destination?.port == url.port ? .allow : .cancel)
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        complete(["error": error.localizedDescription])
    }

    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        complete(["error": error.localizedDescription])
    }

    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard let result = message.body as? [String: Any] else { return }
        if result["type"] as? String == "phase", let phase = result["phase"] as? String {
            if phase.hasSuffix("-idle") {
                idleStart = (ProcessInfo.processInfo.systemUptime, processCPUSeconds())
            } else if let start = idleStart {
                let duration = ProcessInfo.processInfo.systemUptime - start.wall
                idleResourceUsage = ["duration_seconds": duration, "cpu_percent": (processCPUSeconds() - start.cpu) / duration * 100]
                idleStart = nil
            }
            print("PENNEL_BENCHMARK_PHASE \(phase)")
            fflush(stdout)
            return
        }
        complete(result)
    }

    func complete(_ result: [String: Any]) {
        guard !finished else { return }
        finished = true
        var payload = result
        payload["runner_pid"] = ProcessInfo.processInfo.processIdentifier
        payload["os"] = ProcessInfo.processInfo.operatingSystemVersionString
        payload["window_points"] = [980, 640]
        payload["backing_scale"] = window?.backingScaleFactor ?? 0
        payload["display_refresh_hz"] = window?.screen?.maximumFramesPerSecond ?? 0
        payload["host_process_cpu"] = ["cpu_percent": (processCPUSeconds() - startedCPU) / (ProcessInfo.processInfo.systemUptime - started) * 100,
                                        "idle": idleResourceUsage, "excludes_webcontent_and_gpu": true]
        do {
            let data = try JSONSerialization.data(withJSONObject: payload, options: [.prettyPrinted, .sortedKeys])
            try data.write(to: output, options: .atomic)
        } catch {
            fputs("Failed to save benchmark: \(error)\n", stderr)
        }
        webview?.stopLoading()
        NSApplication.shared.terminate(nil)
    }
}

let arguments = CommandLine.arguments
guard arguments.count == 3, let url = URL(string: arguments[1]), url.host == "127.0.0.1" else {
    fputs("Usage: PennelWebKitBenchmark <loopback-url> <output.json>\n", stderr)
    exit(2)
}
MainActor.assumeIsolated {
    let application = NSApplication.shared
    application.setActivationPolicy(.accessory)
    let runner = Runner(url: url, output: URL(fileURLWithPath: arguments[2]))
    application.delegate = runner
    application.run()
}
