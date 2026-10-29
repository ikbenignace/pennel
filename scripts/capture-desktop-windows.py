#!/usr/bin/env python3
"""Capture owned native windows using a dedicated app identity and synthetic data."""

import argparse
from contextlib import closing
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import signal
import sqlite3
import subprocess
import tempfile
import time
import uuid


ROOT = Path(__file__).resolve().parents[1]
CAPTURE_IDENTIFIER = "dev.pennel.Pennel.capture.preview"
FIXTURE_CLOCK = "2026-10-25T09:00:00Z"
DEFAULT_BINARY = ROOT / "apps/desktop/src-tauri/target/release/bundle/macos/Pennel Capture Preview.app/Contents/MacOS/pennel"
CASES = {
    "meeting-notes": ("library", None, "main", "notes"),
    "meeting-transcript": ("library", None, "main", "transcript"),
    "meeting-questions": ("library", None, "main", "ask"),
    "meetings-questions": ("library", "chat", "main", None),
    "history": ("history", None, "main", None),
    "dictation": ("dictation", "home", "main", None),
    "dictation-pad": ("dictation", "pad", "main", None),
    "recording-start": ("live", None, "main", None),
    **{f"settings-{tab}": ("settings", tab, "main", None) for tab in (
        "overview", "providers", "models", "dictation", "meeting-settings", "recording",
        "dictionary", "styles", "appearance", "account", "local",
    )},
    **{f"onboarding-{step}": ("onboarding", step, "onboarding", None) for step in (
        "welcome", "engine", "permissions", "practice",
    )},
}


def fixture_meetings(locale):
    if locale == "en":
        return [
            ("2026-10-25 0900 Weekly planning", "Weekly planning", "2026-10-25T09:00:00+00:00", 18,
             "## Summary\n- The team reviewed this week's planning.\n\n## Decisions\n- Noor will lead the client demo.\n\n## Action items\n- [ ] Sam will share the agenda.",
             "[00:00] **Me:** Shall we review the week?\n[00:06] **Others:** The Friday release is still on.\n[00:15] **Me:** Noor, will you lead the client demo?\n"),
            ("2026-10-24 1400 Project discussion", "Project discussion", "2026-10-24T14:00:00+00:00", 25,
             "## Summary\n- The proposal goes to the client this week.\n\n## Action items\n- [ ] Confirm the next appointment.",
             "[00:00] **Others:** We will finish planning on Monday.\n[00:08] **Me:** I will confirm the next appointment.\n"),
        ]
    return [
        ("2026-10-25 0900 Weekstart", "Weekstart", "2026-10-25T09:00:00+00:00", 18,
         "## Samenvatting\n- Het team overloopt de planning.\n\n## Beslissingen\n- Noor neemt de klantdemo.\n\n## Actiepunten\n- [ ] Sam stuurt de agenda rond.",
         "[00:00] **Ik:** Zullen we de week overlopen?\n[00:06] **Anderen:** De release van vrijdag blijft staan.\n[00:15] **Ik:** Noor, neem jij de klantdemo?\n"),
        ("2026-10-24 1400 Projectoverleg", "Projectoverleg", "2026-10-24T14:00:00+00:00", 25,
         "## Samenvatting\n- Het voorstel gaat deze week naar de klant.\n\n## Actiepunten\n- [ ] Volgende afspraak bevestigen.",
         "[00:00] **Anderen:** We ronden de planning maandag af.\n[00:08] **Ik:** Ik bevestig de volgende afspraak.\n"),
    ]


def write_fixtures(data, locale):
    for identifier, title, start, minutes, summary, transcript in fixture_meetings(locale):
        folder = data / "Meetings" / identifier
        folder.mkdir(parents=True)
        (folder / "meeting.md").write_text(
            f"---\ntitle: {title}\nstart: {start}\nduration_min: {minutes}\ntemplate: general\n---\n\n{summary}\n"
        )
        (folder / "transcript.md").write_text(transcript)
        (folder / "notes.md").write_text("- Planning\n- Next appointment\n" if locale == "en" else "- Planning\n- Volgende afspraak\n")
    rows = (
        ("qa-1", 1792918200000, "com.apple.MobileSMS", "Shall we meet tomorrow at ten?" if locale == "en" else "Zullen we morgen om tien uur afspreken?"),
        ("qa-2", 1792916400000, "com.apple.mail", "I will update the proposal." if locale == "en" else "Ik werk het voorstel bij."),
        ("qa-3", 1792912800000, "com.apple.Notes", "Coffee, milk and bread." if locale == "en" else "Koffie, melk en brood."),
    )
    with closing(sqlite3.connect(data / "history.sqlite")) as connection, connection:
        connection.execute("CREATE TABLE IF NOT EXISTS dictations (id TEXT PRIMARY KEY, created_at_ms INTEGER NOT NULL, app_bundle_id TEXT, raw TEXT NOT NULL, text TEXT NOT NULL, status TEXT NOT NULL, error TEXT, has_audio INTEGER NOT NULL)")
        connection.execute("PRAGMA user_version=1")
        connection.executemany("INSERT INTO dictations VALUES (?, ?, ?, ?, ?, 'done', NULL, 0)",
                               [(identifier, stamp, app, text, text) for identifier, stamp, app, text in rows])


def capture_environment(data):
    environment = os.environ.copy()
    environment.update(PENNEL_WINDOW_CAPTURE="1", PENNEL_CAPTURE_DATA_DIR=str(data))
    return environment


def validate_binary(binary):
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise ValueError("Build Pennel Capture Preview with src-tauri/tauri.capture.conf.json first")
    info_path = binary.parent.parent / "Info.plist"
    with info_path.open("rb") as source:
        info = plistlib.load(source)
    if info.get("CFBundleIdentifier") != CAPTURE_IDENTIFIER:
        raise ValueError("Native captures require the dedicated capture identity")


def validate_window(metadata, pid, label):
    if metadata.get("pid") != pid or metadata.get("label") != label:
        raise ValueError("Window metadata does not belong to this capture process and label")
    window_id = metadata.get("window_id")
    if type(window_id) is not int or window_id <= 0:
        raise ValueError("Capture window has no valid native window ID")
    return window_id


WINDOW_CHECK = r"""
import AppKit
import CoreGraphics
import Foundation

final class FixtureBackdrop: NSWindow {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}

final class FixturePattern: NSView {
    override var isOpaque: Bool { true }
    override func draw(_ dirtyRect: NSRect) {
        NSColor(calibratedRed: 0.76, green: 0.84, blue: 0.91, alpha: 1).setFill()
        bounds.fill()
        let shapes: [(NSColor, NSRect)] = [
            (NSColor(calibratedRed: 0.18, green: 0.43, blue: 0.62, alpha: 1), NSRect(x: bounds.width * 0.12, y: 0, width: bounds.width * 0.19, height: bounds.height)),
            (NSColor(calibratedRed: 0.91, green: 0.53, blue: 0.30, alpha: 1), NSRect(x: bounds.width * 0.34, y: bounds.height * 0.2, width: bounds.width * 0.4, height: bounds.height * 0.45)),
            (NSColor(calibratedRed: 0.34, green: 0.62, blue: 0.49, alpha: 1), NSRect(x: bounds.width * 0.69, y: bounds.height * 0.48, width: bounds.width * 0.28, height: bounds.height * 0.5)),
        ]
        for (color, rect) in shapes {
            color.setFill()
            NSBezierPath(roundedRect: rect, xRadius: 80, yRadius: 80).fill()
        }
        NSColor.white.withAlphaComponent(0.35).setStroke()
        for x in stride(from: CGFloat(0), through: bounds.width, by: 80) {
            let line = NSBezierPath()
            line.move(to: NSPoint(x: x, y: 0))
            line.line(to: NSPoint(x: x + bounds.height * 0.25, y: bounds.height))
            line.lineWidth = 3
            line.stroke()
        }
    }
}

@MainActor
func runBackdrop(_ metadataPath: String) {
    let app = NSApplication.shared
    app.setActivationPolicy(.accessory)
    app.finishLaunching()
    let backdrops = NSScreen.screens.map { screen in
        let window = FixtureBackdrop(contentRect: screen.frame, styleMask: [.borderless], backing: .buffered, defer: false)
        window.title = "Pennel synthetic material backdrop"
        window.isReleasedWhenClosed = false
        window.hidesOnDeactivate = false
        window.ignoresMouseEvents = true
        window.isOpaque = true
        window.collectionBehavior = [.canJoinAllSpaces, .stationary, .ignoresCycle]
        window.contentView = FixturePattern(frame: NSRect(origin: .zero, size: screen.frame.size))
        return window
    }
    var revision = 0
    func present() {
        for window in backdrops {
            window.orderFrontRegardless()
            window.displayIfNeeded()
        }
        revision += 1
        let metadata: [String: Any] = [
            "pid": ProcessInfo.processInfo.processIdentifier,
            "window_ids": backdrops.map(\.windowNumber),
            "revision": revision,
            "activation_policy": app.activationPolicy().rawValue,
            "application_active": app.isActive,
        ]
        if let data = try? JSONSerialization.data(withJSONObject: metadata) {
            try? data.write(to: URL(fileURLWithPath: metadataPath), options: .atomic)
        }
    }
    signal(SIGUSR1, SIG_IGN)
    let source = DispatchSource.makeSignalSource(signal: SIGUSR1, queue: .main)
    source.setEventHandler(handler: present)
    source.resume()
    present()
    withExtendedLifetime(source) { app.run() }
}

if CommandLine.arguments.count == 3 && CommandLine.arguments[1] == "--backdrop" {
    MainActor.assumeIsolated { runBackdrop(CommandLine.arguments[2]) }
    exit(0)
}
let describe = CommandLine.arguments.count > 1 && CommandLine.arguments[1] == "--describe"
let offset = describe ? 2 : 1
guard CommandLine.arguments.count == offset + 2,
      let owner = Int32(CommandLine.arguments[offset]),
      let identifier = Int(CommandLine.arguments[offset + 1]) else { exit(64) }
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
guard let window = windows.first(where: {
    ($0[kCGWindowOwnerPID as String] as? Int32) == owner &&
    ($0[kCGWindowNumber as String] as? Int) == identifier
}) else { exit(1) }
if describe {
    let application = NSRunningApplication(processIdentifier: owner)
    let metadata: [String: Any] = [
        "pid": owner,
        "window_id": identifier,
        "bounds": window[kCGWindowBounds as String] ?? [:],
        "layer": window[kCGWindowLayer as String] ?? 0,
        "alpha": window[kCGWindowAlpha as String] ?? 1,
        "activation_policy": application?.activationPolicy.rawValue ?? -1,
        "application_active": application?.isActive ?? false,
    ]
    let data = try JSONSerialization.data(withJSONObject: metadata)
    print(String(decoding: data, as: UTF8.self))
}
exit(0)
"""


def stop_owned_process(process):
    if process and process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


class NativeBackdrop:
    def __init__(self, helper, metadata_path, log):
        self.helper = helper
        self.metadata_path = metadata_path
        self.log = log
        self.process = None
        self.metadata = None

    def start(self):
        self.metadata_path.unlink(missing_ok=True)
        self.process = subprocess.Popen([str(self.helper), "--backdrop", str(self.metadata_path)],
                                        stdout=self.log, stderr=subprocess.STDOUT)
        self.metadata = self.wait(0)

    def wait(self, after_revision):
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                raise RuntimeError("The owned synthetic backdrop exited")
            try:
                metadata = json.loads(self.metadata_path.read_text())
                if metadata.get("pid") == self.process.pid and metadata.get("revision", 0) > after_revision and metadata.get("window_ids"):
                    return metadata
            except (FileNotFoundError, json.JSONDecodeError):
                pass
            time.sleep(0.05)
        raise RuntimeError("The owned synthetic backdrop did not become ready")

    def present(self):
        if not self.process or self.process.poll() is not None:
            raise RuntimeError("The owned synthetic backdrop is not running")
        self.process.send_signal(signal.SIGUSR1)
        self.metadata = self.wait(self.metadata["revision"])

    def stop(self):
        stop_owned_process(self.process)
        self.process = None


class CaptureSession:
    def __init__(self, binary, data, log, window_check, backdrop=None):
        self.binary = binary
        self.data = data
        self.log = log
        self.window_check = window_check
        self.environment = capture_environment(data)
        self.process = None
        self.backdrop = backdrop

    def stop(self):
        stop_owned_process(self.process)
        self.process = None

    def launch(self, route="workspace", tab=None):
        self.stop()
        if self.backdrop:
            self.backdrop.present()
        for metadata in self.data.glob("capture-*.json"):
            metadata.unlink()
        arguments = [str(self.binary), "--capture-show", route]
        if tab:
            arguments.append(tab)
        self.process = subprocess.Popen(arguments, env=self.environment, stdout=self.log, stderr=subprocess.STDOUT)

    def run(self, *arguments):
        if not self.process or self.process.poll() is not None:
            raise RuntimeError("The owned capture app is not running")
        subprocess.run([str(self.binary), *arguments], env=self.environment, stdout=self.log,
                       stderr=subprocess.STDOUT, check=True, timeout=15)

    def window(self, label):
        if not self.process or self.process.poll() is not None:
            raise RuntimeError("The owned capture app exited; another capture instance may be running")
        metadata = json.loads((self.data / f"capture-window-{label}.json").read_text())
        identifier = validate_window(metadata, self.process.pid, label)
        subprocess.run([str(self.window_check), str(self.process.pid), str(identifier)],
                       check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=3)
        return identifier

    def describe_window(self, label):
        identifier = self.window(label)
        result = subprocess.run([str(self.window_check), "--describe", str(self.process.pid), str(identifier)],
                                check=True, capture_output=True, text=True, timeout=3)
        return json.loads(result.stdout)

    def inspect(self, label, timeout=25):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            try:
                self.window(label)
                break
            except (FileNotFoundError, json.JSONDecodeError, subprocess.CalledProcessError):
                time.sleep(0.2)
        else:
            raise RuntimeError(f"The owned {label} window did not appear")
        request = uuid.uuid4().hex
        self.run("--capture-inspect", label, request)
        while time.monotonic() < deadline:
            try:
                self.window(label)
                snapshot = json.loads((self.data / f"capture-state-{label}.json").read_text())
                if snapshot["request_id"] == request:
                    if snapshot.get("error"):
                        raise RuntimeError(snapshot["error"])
                    if snapshot.get("ready"):
                        return snapshot
                    request = uuid.uuid4().hex
                    self.run("--capture-inspect", label, request)
            except (FileNotFoundError, json.JSONDecodeError, subprocess.CalledProcessError):
                self.run("--capture-inspect", label, request)
            time.sleep(0.2)
        raise RuntimeError(f"The owned {label} window did not become ready")

    def initialize(self, locale, appearance):
        self.launch()
        self.inspect("main")
        update = {"appearance": appearance, "ui_language": locale, "history_retention_days": 0,
                  "auto_detect": False, "record_shortcut_enabled": False, "calendar_enabled": False,
                  "cloud_sync": False, "claude_access": False, "dictionary": ["Pennel", "Studio Noord", "Kubernetes"]}
        self.run("--capture-eval", "main", "(async()=>{localStorage.clear();sessionStorage.clear();const i=window.__TAURI_INTERNALS__;const s=(await i.invoke('app_info')).settings;Object.assign(s," + json.dumps(update) + ");s.onboarding.status='completed';await i.invoke('save_settings',{settings:s});})()")
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            try:
                settings = json.loads((self.data / "settings.json").read_text())
                if all(settings.get(key) == value for key, value in update.items()) and settings["onboarding"]["status"] == "completed":
                    self.stop()
                    return
            except (FileNotFoundError, json.JSONDecodeError):
                pass
            time.sleep(0.1)
        raise RuntimeError("Synthetic capture settings were not saved")

    def capture(self, case, destination, size):
        route, tab, label, meeting_tab = CASES[case]
        self.launch(route, tab)
        self.inspect(label)
        if size:
            self.run("--capture-size", label, str(size[0]), str(size[1]))
        if meeting_tab:
            self.run("--capture-eval", label, f"document.querySelector('[data-tab=\"{meeting_tab}\"]')?.click()")
        time.sleep(0.5)
        snapshot = self.inspect(label)
        if meeting_tab and snapshot.get("selected_tab") != meeting_tab:
            raise RuntimeError(f"Meeting tab did not become selected: {meeting_tab}")
        identifier = self.window(label)
        native_window = self.describe_window(label)
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.unlink(missing_ok=True)
        try:
            subprocess.run(["/usr/sbin/screencapture", "-x", "-o", "-l", str(identifier), str(destination)],
                           check=True, stdout=self.log, stderr=subprocess.STDOUT, timeout=12)
        except subprocess.TimeoutExpired:
            if not destination.is_file() or not destination.stat().st_size:
                raise
        if not destination.is_file() or not destination.stat().st_size:
            raise RuntimeError(f"Native capture failed: {case}")
        snapshot.pop("request_id", None)
        snapshot["window_label"] = label
        snapshot["window_id"] = identifier
        snapshot["native_window"] = native_window
        snapshot["backdrop"] = self.backdrop.metadata if self.backdrop else None
        snapshot["horizontal_overflow"] = snapshot["scroll_width"] > snapshot["viewport"][0] + 1
        return snapshot


def parse_arguments(arguments=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "build/visual-parity/windows")
    parser.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    parser.add_argument("--appearance", choices=("both", "light", "dark"), default="both")
    parser.add_argument("--locale", choices=("both", "en", "nl"), default="both")
    parser.add_argument("--case", choices=tuple(CASES), action="append", dest="cases")
    parser.add_argument("--size", help="Logical window size, for example 760x520")
    parser.add_argument("--no-backdrop", action="store_true", help="Disable the owned synthetic native backdrop")
    parser.add_argument("--list", action="store_true")
    options = parser.parse_args(arguments)
    if options.size:
        try:
            options.size = tuple(int(value) for value in options.size.split("x"))
            if len(options.size) != 2 or any(not 200 <= value <= 4096 for value in options.size):
                raise ValueError()
        except ValueError:
            parser.error("--size must contain two dimensions from 200 to 4096")
    options.binary = options.binary.resolve()
    options.output = options.output.resolve()
    return options


def main(arguments=None):
    options = parse_arguments(arguments)
    if options.list:
        print("\n".join(CASES))
        return
    if platform.system() != "Darwin":
        raise SystemExit("Native window captures require macOS")
    validate_binary(options.binary)
    options.output.mkdir(parents=True, exist_ok=True)
    manifest_path = options.output / "manifest.json"
    manifest = {
        "complete": False,
        "capture_method": "Owned native macOS application windows; synthetic data, dedicated capture identity, and owned synthetic native backdrop" if not options.no_backdrop else "Owned native macOS application windows; synthetic data and dedicated capture identity; backdrop disabled",
        "owned_backdrop": not options.no_backdrop,
        "fixture_clock": FIXTURE_CLOCK,
        "captured_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "binary_sha256": hashlib.sha256(options.binary.read_bytes()).hexdigest(),
        "capture_identifier": CAPTURE_IDENTIFIER,
        "os_version": platform.mac_ver()[0],
        "images": [],
    }
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
    appearances = ("light", "dark") if options.appearance == "both" else (options.appearance,)
    locales = ("en", "nl") if options.locale == "both" else (options.locale,)
    with tempfile.TemporaryDirectory(prefix="pennel-window-capture-") as temporary:
        work = Path(temporary)
        swift = work / "owned-window.swift"
        swift.write_text(WINDOW_CHECK)
        window_check = work / "owned-window"
        subprocess.run(["swiftc", "-O", str(swift), "-o", str(window_check)], check=True)
        with (options.output / "app.log").open("w") as log:
            backdrop = None if options.no_backdrop else NativeBackdrop(window_check, work / "backdrop.json", log)
            try:
                if backdrop:
                    backdrop.start()
                for locale in locales:
                    for appearance in appearances:
                        data = work / f"data-{locale}-{appearance}"
                        data.mkdir()
                        session = CaptureSession(options.binary, data, log, window_check, backdrop)
                        try:
                            session.initialize(locale, appearance)
                            write_fixtures(data, locale)
                            for case in options.cases or CASES:
                                relative = Path(locale) / appearance / f"{case}.png"
                                snapshot = session.capture(case, options.output / relative, options.size)
                                manifest["images"].append({"case": case, "locale": locale, "appearance": appearance,
                                                           "file": str(relative), **snapshot})
                                manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
                                if snapshot["document_language"] != locale:
                                    raise RuntimeError("The captured interface language does not match the requested locale")
                                if snapshot["horizontal_overflow"]:
                                    raise RuntimeError(f"Horizontal overflow in native capture: {case}")
                                print(relative, flush=True)
                        finally:
                            session.stop()
            finally:
                if backdrop:
                    backdrop.stop()
    manifest["complete"] = True
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")


if __name__ == "__main__":
    main()
