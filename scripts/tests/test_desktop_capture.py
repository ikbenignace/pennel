import importlib.util
from contextlib import closing
import json
import os
from pathlib import Path
import plistlib
import sqlite3
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("capture_windows", ROOT / "scripts/capture-desktop-windows.py")
capture = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(capture)


class NativeCaptureIsolationTests(unittest.TestCase):
    def test_capture_environment_keeps_user_home_and_uses_own_data_root(self):
        with mock.patch.dict(os.environ, {"HOME": "/Users/original", "PENNEL_CAPTURE_DATA_DIR": "/stale"}):
            environment = capture.capture_environment(Path("/tmp/owned-fixture"))
        self.assertEqual(environment["HOME"], "/Users/original")
        self.assertEqual(environment["PENNEL_CAPTURE_DATA_DIR"], "/tmp/owned-fixture")
        self.assertEqual(environment["PENNEL_WINDOW_CAPTURE"], "1")

    def test_installed_preview_and_production_binaries_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            contents = Path(temporary) / "Pennel.app/Contents"
            binary = contents / "MacOS/pennel"
            binary.parent.mkdir(parents=True)
            binary.write_text("#!/bin/sh\nexit 0\n")
            binary.chmod(0o755)
            for identifier in ("dev.pennel.Pennel", "dev.pennel.Pennel.preview", capture.CAPTURE_IDENTIFIER):
                (contents / "Info.plist").write_bytes(plistlib.dumps({"CFBundleIdentifier": identifier}))
                if identifier == capture.CAPTURE_IDENTIFIER:
                    capture.validate_binary(binary)
                else:
                    with self.assertRaisesRegex(ValueError, "dedicated capture identity"):
                        capture.validate_binary(binary)

    def test_window_id_must_belong_to_the_launched_process_and_label(self):
        self.assertEqual(capture.validate_window({"pid": 123, "label": "main", "window_id": 42}, 123, "main"), 42)
        for invalid in (
            {"pid": 999, "label": "main", "window_id": 42},
            {"pid": 123, "label": "onboarding", "window_id": 42},
            {"pid": 123, "label": "main", "window_id": -1},
            {"pid": 123, "label": "main", "window_id": True},
        ):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                capture.validate_window(invalid, 123, "main")

    def test_english_and_dutch_fixtures_are_stable_and_stay_inside_the_fixture(self):
        for locale in ("en", "nl"):
            with self.subTest(locale=locale), tempfile.TemporaryDirectory() as temporary:
                data = Path(temporary) / "synthetic-data"
                data.mkdir()
                capture.write_fixtures(data, locale)
                meetings = capture.fixture_meetings(locale)
                self.assertEqual(len(list((data / "Meetings").iterdir())), len(meetings))
                for identifier, title, start, minutes, summary, transcript in meetings:
                    folder = data / "Meetings" / identifier
                    self.assertIn(title, (folder / "meeting.md").read_text())
                    self.assertIn(start, (folder / "meeting.md").read_text())
                    self.assertEqual((folder / "transcript.md").read_text(), transcript)
                    self.assertTrue(folder.is_relative_to(data))
                with closing(sqlite3.connect(data / "history.sqlite")) as connection:
                    rows = connection.execute("SELECT id, created_at_ms, text FROM dictations ORDER BY id").fetchall()
                self.assertEqual([row[0] for row in rows], ["qa-1", "qa-2", "qa-3"])
                self.assertEqual(rows[0][1], 1792918200000)
                self.assertIn("Shall" if locale == "en" else "Zullen", rows[0][2])

    def test_capture_config_is_a_distinct_preview_and_cannot_emit_updates(self):
        config = json.loads((ROOT / "apps/desktop/src-tauri/tauri.capture.conf.json").read_text())
        self.assertEqual(config["identifier"], capture.CAPTURE_IDENTIFIER)
        self.assertTrue(config["identifier"].endswith(".preview"))
        self.assertFalse(config["bundle"]["createUpdaterArtifacts"])

    def test_only_a_validated_window_id_is_passed_to_screencapture(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary)
            destination = path / "main.png"
            session = capture.CaptureSession(Path("/capture-binary"), path, None, Path("/window-check"))
            snapshot = {"request_id": "ready", "ready": True, "selected_tab": "notes", "viewport": [980, 640], "scroll_width": 980}

            def screenshot(arguments, **_kwargs):
                destination.write_bytes(b"owned-window-pixels")
                self.assertEqual(arguments, ["/usr/sbin/screencapture", "-x", "-o", "-l", "42", str(destination)])

            with mock.patch.object(session, "launch"), mock.patch.object(session, "inspect", return_value=snapshot), mock.patch.object(session, "run"), mock.patch.object(session, "window", return_value=42), mock.patch.object(session, "describe_window", return_value={"window_id": 42, "activation_policy": 1}), mock.patch.object(capture.time, "sleep"), mock.patch.object(capture.subprocess, "run", side_effect=screenshot):
                result = session.capture("meeting-notes", destination, None)
            self.assertEqual(result["window_label"], "main")
            self.assertFalse(result["horizontal_overflow"])
            self.assertIsNone(result["backdrop"])
            self.assertEqual(result["native_window"]["activation_policy"], 1)

    def test_unowned_or_exited_process_cannot_receive_followup_commands(self):
        session = capture.CaptureSession(Path("/capture-binary"), Path("/tmp/fixture"), None, Path("/window-check"))
        with mock.patch.object(capture.subprocess, "run") as run, self.assertRaisesRegex(RuntimeError, "not running"):
            session.run("--capture-show", "workspace")
        run.assert_not_called()

    def test_native_backdrop_is_enabled_by_default_and_can_be_disabled(self):
        self.assertFalse(capture.parse_arguments([]).no_backdrop)
        self.assertTrue(capture.parse_arguments(["--no-backdrop"]).no_backdrop)

    def test_only_the_owned_backdrop_process_receives_reorder_and_cleanup(self):
        with tempfile.TemporaryDirectory() as temporary:
            process = mock.Mock(pid=456)
            process.poll.return_value = None
            backdrop = capture.NativeBackdrop(Path("/owned-helper"), Path(temporary) / "metadata.json", None)
            metadata = {"pid": 456, "window_ids": [41], "revision": 1}
            reordered = {**metadata, "revision": 2}
            with mock.patch.object(capture.subprocess, "Popen", return_value=process) as start, mock.patch.object(backdrop, "wait", side_effect=[metadata, reordered]):
                backdrop.start()
                backdrop.present()
                backdrop.stop()
            self.assertEqual(start.call_args.args[0], ["/owned-helper", "--backdrop", str(backdrop.metadata_path)])
            process.send_signal.assert_called_once_with(capture.signal.SIGUSR1)
            process.terminate.assert_called_once_with()
            process.kill.assert_not_called()
            self.assertIsNone(backdrop.process)

    def test_capture_launch_raises_backdrop_before_the_new_owned_app_window(self):
        backdrop = mock.Mock()
        session = capture.CaptureSession(Path("/capture-binary"), Path("/tmp/fixture"), None, Path("/window-check"), backdrop)
        order = []
        backdrop.present.side_effect = lambda: order.append("backdrop")
        with mock.patch.object(Path, "glob", return_value=[]), mock.patch.object(capture.subprocess, "Popen", side_effect=lambda *args, **kwargs: order.append("app")):
            session.launch()
        self.assertEqual(order, ["backdrop", "app"])

    def test_backdrop_readiness_rejects_metadata_from_another_process(self):
        process = mock.Mock(pid=456)
        process.poll.return_value = None
        backdrop = capture.NativeBackdrop(Path("/owned-helper"), Path("/tmp/metadata.json"), None)
        backdrop.process = process
        stale = json.dumps({"pid": 999, "revision": 2, "window_ids": [41]})
        with mock.patch.object(Path, "read_text", return_value=stale), mock.patch.object(capture.time, "monotonic", side_effect=[0, 0, 9]), mock.patch.object(capture.time, "sleep"), self.assertRaisesRegex(RuntimeError, "did not become ready"):
            backdrop.wait(0)


if __name__ == "__main__":
    unittest.main()
