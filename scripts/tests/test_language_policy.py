"""Keep app-owned diagnostic producers English without policing user content."""

import pathlib
import re
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
LITERAL = re.compile(r'"(?:\\.|[^"\\])*"')
DUTCH_DIAGNOSTIC = re.compile(
    r"\b(?:mislukt|geweigerd|gesloten|ongeldig|ongeldige|sprekerherkenning|opkuis|sleutelhanger|toestemming|"
    r"niet beschikbaar|niet ondersteund|kan niet|kon niet|niet worden|"
    r"wordt al|wordt nog|wordt afgesloten)\b",
    re.IGNORECASE,
)
COMPATIBILITY_LITERALS = {
    "core/src/meetings.rs": {'"_(transcriptie mislukt)_"'},
    "apps/desktop/src-tauri/src/commands.rs": {'"Bewaren mislukt: "'},
}


def runtime_source(text):
    return re.split(r"#\[cfg\(test\)\]\s*mod\s+\w+\s*\{", text, maxsplit=1)[0]


class LanguagePolicyTests(unittest.TestCase):
    def test_test_helpers_do_not_hide_later_runtime_diagnostics(self):
        source = '#[cfg(test)] fn helper() {}\nfn runtime() { error("mislukt"); }\n#[cfg(test)] mod tests { assert!(true); }'
        self.assertIn('error("mislukt")', runtime_source(source))
        self.assertNotIn("assert!(true)", runtime_source(source))

    def test_runtime_diagnostic_producers_use_english(self):
        violations = []
        paths = [
            *ROOT.joinpath("core/src").rglob("*.rs"),
            *ROOT.joinpath("core/speakers/src").rglob("*.rs"),
            *ROOT.joinpath("apps/desktop/src-tauri/src").rglob("*.rs"),
            *ROOT.joinpath("apps/macos-platform/Sources").rglob("*.swift"),
        ]
        for path in paths:
            relative = path.relative_to(ROOT).as_posix()
            text = runtime_source(path.read_text())
            allowed = COMPATIBILITY_LITERALS.get(relative, set())
            for match in LITERAL.finditer(text):
                if match.group() in allowed or not DUTCH_DIAGNOSTIC.search(match.group()):
                    continue
                line = text.count("\n", 0, match.start()) + 1
                violations.append(f"{relative}:{line}: {match.group()}")
        self.assertEqual(violations, [], "\n".join(violations))


if __name__ == "__main__":
    unittest.main()
