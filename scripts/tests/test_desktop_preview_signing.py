import importlib.util
import io
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).parents[1] / "preview-signing-identity.py"
SPEC = importlib.util.spec_from_file_location("preview_signing_identity", SCRIPT)
signing = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = signing
SPEC.loader.exec_module(signing)

FIRST = "A" * 40
SECOND = "B" * 40
LOCAL = "C" * 40
SECURITY_OUTPUT = f'''  1) {FIRST} "Apple Development: First Developer (TEAM1)"
  2) {SECOND} "Apple Development: Second Developer (TEAM2)"
     2 valid identities found
'''


class PreviewSigningIdentityTests(unittest.TestCase):
    def setUp(self):
        self.identities = signing.parse_identities(SECURITY_OUTPUT)

    def test_first_existing_apple_development_certificate_is_used_without_a_cache(self):
        selection = signing.select_identity(self.identities)
        self.assertEqual(selection.identity, FIRST)
        self.assertEqual(selection.sha1, FIRST)

    def test_cached_certificate_survives_a_change_in_identity_order(self):
        selection = signing.select_identity(list(reversed(self.identities)), cached_sha=FIRST.lower())
        self.assertEqual(selection.identity, FIRST)

    def test_stale_cache_falls_back_to_a_current_valid_certificate(self):
        selection = signing.select_identity(self.identities, cached_sha="D" * 40)
        self.assertEqual(selection.identity, FIRST)

    def test_known_local_certificates_precede_apple_development(self):
        local = signing.Identity(LOCAL, "Pennel Local Dev")
        selection = signing.select_identity(self.identities + [local])
        self.assertEqual(selection.identity, LOCAL)
        self.assertEqual(signing.select_identity(self.identities + [local], cached_sha=SECOND).identity, SECOND)

    def test_apple_override_precedes_pennel_and_cache(self):
        selection = signing.select_identity(
            self.identities, cached_sha=SECOND,
            apple_override="Apple Development: First Developer (TEAM1)", pennel_override=SECOND,
        )
        self.assertEqual(selection.identity, "Apple Development: First Developer (TEAM1)")
        self.assertEqual(selection.sha1, FIRST)

    def test_pennel_override_and_explicit_adhoc_are_preserved(self):
        self.assertEqual(signing.select_identity(self.identities, pennel_override=SECOND).identity, SECOND)
        adhoc = signing.select_identity(self.identities, cached_sha=FIRST, apple_override="-")
        self.assertEqual(adhoc.identity, "-")
        self.assertIsNone(adhoc.sha1)

    def test_unknown_explicit_identity_is_not_replaced_silently(self):
        selection = signing.select_identity(self.identities, apple_override="Requested release identity")
        self.assertEqual(selection.identity, "Requested release identity")
        self.assertIsNone(selection.sha1)

    def test_missing_certificate_falls_back_to_adhoc(self):
        selection = signing.select_identity([])
        self.assertEqual(selection.identity, "-")
        self.assertIsNone(selection.sha1)

    def test_parser_rejects_invalid_or_unrecognized_identity_entries(self):
        output = SECURITY_OUTPUT + f'  3) {LOCAL} "Expired" (CSSMERR_TP_CERT_EXPIRED)\n  4) badhash "Invalid"\n'
        self.assertEqual([identity.sha1 for identity in signing.parse_identities(output)], [FIRST, SECOND])

    def test_cli_reads_valid_identities_and_atomically_persists_only_the_public_hash(self):
        with tempfile.TemporaryDirectory() as directory:
            cache = Path(directory) / "preview-signing-identity.sha1"
            output = io.StringIO()
            with patch.dict(os.environ, {}, clear=True), patch.object(
                signing.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, SECURITY_OUTPUT, "")
            ) as security, patch("sys.stdout", output):
                self.assertEqual(signing.main(["--cache", str(cache)]), 0)
            self.assertEqual(output.getvalue(), FIRST + "\n")
            self.assertEqual(cache.read_text(), FIRST + "\n")
            self.assertEqual(list(cache.parent.iterdir()), [cache])
            self.assertEqual(security.call_args.args[0], ["security", "find-identity", "-v", "-p", "codesigning"])

    def test_failed_security_lookup_reports_adhoc_fallback(self):
        with tempfile.TemporaryDirectory() as directory:
            cache = Path(directory) / "preview-signing-identity.sha1"
            output, errors = io.StringIO(), io.StringIO()
            with patch.dict(os.environ, {}, clear=True), patch.object(
                signing.subprocess, "run", return_value=subprocess.CompletedProcess([], 1, "", "unavailable")
            ), patch("sys.stdout", output), patch("sys.stderr", errors):
                self.assertEqual(signing.main(["--cache", str(cache)]), 0)
            self.assertEqual(output.getvalue(), "-\n")
            self.assertIn("ad-hoc", errors.getvalue())
            self.assertFalse(cache.exists())


if __name__ == "__main__":
    unittest.main()
