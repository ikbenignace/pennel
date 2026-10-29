#!/usr/bin/env python3
"""Generate the shared CSS and Swift theme from Pennel's design tokens."""

from __future__ import annotations

import argparse
import copy
from html import escape
import json
import math
from pathlib import Path
import re
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
TOKENS = ROOT / "assets/brand/tokens.json"
CSS = ROOT / "apps/desktop/src/theme.generated.css"
SWIFT = ROOT / "apps/macos/Sources/PennelKit/ThemeTokens.generated.swift"
PILL_METRICS = ROOT / "apps/desktop/src/pill-metrics.generated.ts"
ICON_DIRECTORY = ROOT / "assets/ui/icons"
ICON_MANIFEST = ICON_DIRECTORY / "manifest.json"
SWIFT_ICONS = ROOT / "apps/macos/Sources/PennelKit/Icons.generated.swift"
TOKEN_NAME = re.compile(r"^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$")
HEX_COLOR = re.compile(r"^#[0-9a-fA-F]{6}(?:[0-9a-fA-F]{2})?$")
PILL_COUNTS = {"preview-lines", "preview-limit"}
CONTRAST_PAIRS = (
    ("foreground", "surface"),
    ("foreground", "sidebar"),
    ("foreground", "selection"),
    ("muted", "surface"),
    ("muted", "subtle"),
    ("muted", "field"),
    ("muted", "sidebar"),
    ("muted", "selection"),
    ("button-text", "button"),
    ("accent", "surface"),
    ("success", "surface"),
    ("success", "subtle"),
    ("disabled-text", "disabled"),
    ("pill-text", "pill-background"),
    ("pill-accent", "pill-background"),
    *((f"voice-{number}", "surface") for number in range(1, 5)),
)


def channels(value: str) -> tuple[float, float, float]:
    return tuple(int(value[index : index + 2], 16) / 255 for index in (1, 3, 5))


def luminance(value: str) -> float:
    linear = tuple(channel / 12.92 if channel <= 0.04045 else ((channel + 0.055) / 1.055) ** 2.4 for channel in channels(value))
    return sum(channel * coefficient for channel, coefficient in zip(linear, (0.2126, 0.7152, 0.0722)))


def contrast(foreground: str, background: str) -> float:
    first, second = sorted((luminance(foreground), luminance(background)), reverse=True)
    return (first + 0.05) / (second + 0.05)


def composite(foreground: str, background: str) -> str:
    alpha = int(foreground[7:9], 16) / 255 if len(foreground) == 9 else 1
    blended = [round((front * alpha + back * (1 - alpha)) * 255)
               for front, back in zip(channels(foreground), channels(background))]
    return "#" + "".join(f"{channel:02X}" for channel in blended)


def validate(tokens: dict) -> None:
    if tokens.get("schema") != 1:
        raise ValueError("unsupported theme token schema")
    colors = tokens.get("colors", {})
    required = {name for pair in CONTRAST_PAIRS for name in pair}
    required.update(("control-edge", "chrome", "navigation-backing", "navigation-muted", "border", "hover", "focus-ring", "shadow", "key-shadow", "brand-paper", "brand-ink", "wave"))
    if missing := required - colors.keys():
        raise ValueError(f"missing semantic colors: {', '.join(sorted(missing))}")
    for name, pair in colors.items():
        if not TOKEN_NAME.fullmatch(name) or set(pair) != {"light", "dark"}:
            raise ValueError(f"invalid color token: {name}")
        for value in pair.values():
            if not isinstance(value, str) or not HEX_COLOR.fullmatch(value):
                raise ValueError(f"invalid hex color for {name}: {value}")
    for name, target in tokens.get("aliases", {}).items():
        if not TOKEN_NAME.fullmatch(name) or name in colors or target not in colors:
            raise ValueError(f"invalid theme alias: {name}")
    for foreground, background in CONTRAST_PAIRS:
        for appearance in ("light", "dark"):
            ratio = contrast(colors[foreground][appearance], colors[background][appearance])
            if ratio < 4.5:
                raise ValueError(f"{appearance}: {foreground} on {background} has {ratio:.2f}:1 contrast; expected 4.5:1")
    for name in ("pill-background", "pill-text", "pill-accent"):
        if colors[name]["light"].lower() != colors[name]["dark"].lower():
            raise ValueError(f"{name} must remain stable across appearances")
    for appearance in ("light", "dark"):
        for backdrop in ("#000000", "#FFFFFF"):
            backing = composite(colors["navigation-backing"][appearance], backdrop)
            if any(contrast(colors[role][appearance], backing) < 4.5 for role in ("foreground", "navigation-muted")):
                raise ValueError(f"{appearance}: navigation text must retain 4.5:1 contrast over either backdrop extreme")
    for section in ("spacing", "radii", "desktop"):
        for name, value in tokens[section].items():
            if not (str(name).isdigit() or TOKEN_NAME.fullmatch(name)):
                raise ValueError(f"invalid {section} token name: {name}")
            if not isinstance(value, (int, float)) or not math.isfinite(value) or value < 0:
                raise ValueError(f"invalid {section} token: {name}")
    for name, value in tokens["pill"].items():
        if not TOKEN_NAME.fullmatch(name) or isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) or value <= 0:
            raise ValueError(f"invalid pill metric: {name}")
        if name in PILL_COUNTS and not isinstance(value, int):
            raise ValueError(f"invalid pill count: {name}")
    typography = tokens["typography"]
    if typography["ui"] != "Figtree" or typography["display"] != "Instrument Serif":
        raise ValueError("the bundled Pennel font families must be retained")
    if typography.get("interface", "brand") not in ("brand", "system"):
        raise ValueError("typography interface must be brand or system")
    if typography.get("macos-chrome", "brand") not in ("brand", "system"):
        raise ValueError("macOS chrome typography must be brand or system")
    if not tokens["desktop"]["sidebar-min-width"] <= tokens["desktop"]["sidebar-width"] <= tokens["desktop"]["sidebar-max-width"]:
        raise ValueError("desktop sidebar width must be within its bounds")
    for name, value in typography["sizes"].items():
        if not TOKEN_NAME.fullmatch(name) or not isinstance(value, (int, float)) or not 11 <= value <= 48:
            raise ValueError(f"invalid typography size: {name}")


def swift_name(name: str) -> str:
    words = name.split("-")
    return words[0] + "".join(word[:1].upper() + word[1:] for word in words[1:])


def scalar(value: int | float) -> str:
    return f"{value:g}"


def render_css(tokens: dict) -> str:
    lines = ["/* Generated by scripts/generate-theme.py from assets/brand/tokens.json. */", ":root {", "  color-scheme: light;"]
    for name, pair in tokens["colors"].items():
        lines.append(f"  --{name}: {pair['light'].lower()};")
    for name, target in tokens["aliases"].items():
        lines.append(f"  --{name}: var(--{target});")
    for name, value in tokens["spacing"].items():
        lines.append(f"  --space-{name}: {scalar(value)}px;")
    for name, value in tokens["radii"].items():
        lines.append(f"  --radius-{name}: {scalar(value)}px;")
    for name, value in tokens["desktop"].items():
        lines.append(f"  --desktop-{name}: {scalar(value)}px;")
    for name, value in tokens["pill"].items():
        unit = "" if name in PILL_COUNTS else "px"
        lines.append(f"  --pill-{name}: {scalar(value)}{unit};")
    brand_ui = f"\"{tokens['typography']['ui']}\", -apple-system, BlinkMacSystemFont, \"Segoe UI\", system-ui, sans-serif"
    # "system" follows each platform's own UI font (SF, Segoe UI, Cantarell/Ubuntu); "brand" is Figtree, as in the original app.
    system_ui = "-apple-system, BlinkMacSystemFont, \"Segoe UI\", system-ui, Cantarell, Ubuntu, \"Noto Sans\", sans-serif"
    interface = system_ui if tokens["typography"].get("interface") == "system" else brand_ui
    lines.extend((
        f"  --ui: {interface};",
        "  --chrome-ui: var(--ui);",
        f"  --brand-ui: {brand_ui};",
        f"  --serif: \"{tokens['typography']['display']}\", Georgia, serif;",
    ))
    for name, value in tokens["typography"]["sizes"].items():
        lines.append(f"  --font-{name}: {scalar(value)}px;")
    lines.extend(("}", "", "/* System appearance uses a media query supported by the minimum WebKit. */", "@media (prefers-color-scheme: dark) {", "  :root {", "    color-scheme: dark;"))
    for name, pair in tokens["colors"].items():
        lines.append(f"    --{name}: {pair['dark'].lower()};")
    lines.extend(("  }", "}", ""))
    for appearance in ("light", "dark"):
        lines.extend((f':root[data-appearance="{appearance}"], :root[data-theme="{appearance}"] {{', f"  color-scheme: {appearance};"))
        for name, pair in tokens["colors"].items():
            lines.append(f"  --{name}: {pair[appearance].lower()};")
        lines.extend(("}", ""))
    if tokens["typography"].get("macos-chrome") == "system":
        lines.extend((':root[data-platform="macos"] {', f"  --chrome-ui: {system_ui};", "}", ""))
    return "\n".join(lines)


def swift_color_arguments(pair: dict) -> str:
    arguments = [f"light: 0x{pair['light'][1:7].upper()}", f"dark: 0x{pair['dark'][1:7].upper()}"]
    for appearance in ("light", "dark"):
        value = pair[appearance]
        if len(value) == 9:
            arguments.append(f"{appearance}Alpha: {int(value[7:9], 16) / 255:.8f}")
    return ", ".join(arguments)


def render_swift(tokens: dict) -> str:
    lines = [
        "// Generated by scripts/generate-theme.py from assets/brand/tokens.json.",
        "import Foundation", "", "public enum ThemeTokens {",
        "    public struct ColorPair: Sendable {",
        "        public let light: UInt32", "        public let dark: UInt32",
        "        public let lightAlpha: Double", "        public let darkAlpha: Double", "",
        "        public init(light: UInt32, dark: UInt32, lightAlpha: Double = 1, darkAlpha: Double = 1) {",
        "            self.light = light", "            self.dark = dark",
        "            self.lightAlpha = lightAlpha", "            self.darkAlpha = darkAlpha",
        "        }", "    }", "",
    ]
    for name, pair in tokens["colors"].items():
        lines.append(f"    public static let {swift_name(name)} = ColorPair({swift_color_arguments(pair)})")
    for name, target in tokens["aliases"].items():
        lines.append(f"    public static let {swift_name(name)} = {swift_name(target)}")
    lines.extend(("}", "", "public enum ThemeSpacing {"))
    for name, value in tokens["spacing"].items():
        lines.append(f"    public static let step{name}: CGFloat = {scalar(value)}")
    for name, step in (("xs", "1"), ("sm", "2"), ("md", "3"), ("lg", "4"), ("xl", "6"), ("xxl", "8"), ("content", "10")):
        lines.append(f"    public static let {name} = step{step}")
    lines.extend(("}", "", "public enum ThemeRadius {"))
    for name, value in tokens["radii"].items():
        lines.append(f"    public static let {swift_name(name)}: CGFloat = {scalar(value)}")
    lines.extend(("}", "", "public enum ThemePill {"))
    for name, value in tokens["pill"].items():
        kind = "Int" if name in PILL_COUNTS else "CGFloat"
        lines.append(f"    public static let {swift_name(name)}: {kind} = {scalar(value)}")
    lines.extend(("}", "", "public enum ThemeDesktop {"))
    for name, value in tokens["desktop"].items():
        lines.append(f"    public static let {swift_name(name)}: CGFloat = {scalar(value)}")
    lines.extend(("}", "", "public enum ThemeTypography {"))
    lines.append(f"    public static let ui = \"{tokens['typography']['ui']}\"")
    lines.append(f"    public static let display = \"{tokens['typography']['display']}\"")
    lines.append(f"    public static let usesSystemInterface = {str(tokens['typography'].get('interface') == 'system').lower()}")
    for name, value in tokens["typography"]["sizes"].items():
        lines.append(f"    public static let {swift_name(name)}: CGFloat = {scalar(value)}")
    lines.extend(("}", ""))
    return "\n".join(lines)


def render_pill_metrics(tokens: dict) -> str:
    lines = ["// Generated by scripts/generate-theme.py from assets/brand/tokens.json.", "export const PILL_METRICS = {"]
    for name, value in tokens["pill"].items():
        lines.append(f"  {swift_name(name)}: {scalar(value)},")
    lines.extend(("} as const;", ""))
    return "\n".join(lines)


def icon_filename(name: str) -> str:
    return re.sub(r"(?<!^)([A-Z])", r"-\1", name).lower()


def validate_icons(manifest: dict) -> None:
    if manifest.get("schema") != 1 or manifest.get("viewBox") != "0 0 24 24" or manifest.get("strokeWidth") != 1.7:
        raise ValueError("unsupported icon geometry")
    if not manifest.get("icons"):
        raise ValueError("the shared icon set is empty")
    filenames = set()
    for name, paths in manifest["icons"].items():
        if not re.fullmatch(r"[a-z][a-zA-Z0-9]*", name) or not paths:
            raise ValueError(f"invalid icon name: {name}")
        filename = icon_filename(name)
        if filename in filenames:
            raise ValueError(f"duplicate icon filename: {filename}")
        filenames.add(filename)
        for path in paths:
            if not isinstance(path, str) or not re.fullmatch(r"[MmZzLlHhVvCcSsQqTtAaEe0-9+.,\s-]+", path):
                raise ValueError(f"invalid SVG path: {name}")


def render_icon(manifest: dict, name: str) -> str:
    paths = "".join(f'<path d="{escape(path, quote=True)}"/>' for path in manifest["icons"][name])
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{manifest["viewBox"]}" width="24" height="24" fill="none" stroke="currentColor" stroke-width="{manifest["strokeWidth"]}" stroke-linecap="round" stroke-linejoin="round">{paths}</svg>\n'


def render_swift_icons(manifest: dict) -> str:
    lines = [
        "// Generated by scripts/generate-theme.py from assets/ui/icons/manifest.json.",
        "public enum PennelIcon: String, CaseIterable, Sendable {",
    ]
    for name in manifest["icons"]:
        lines.append(f'    case {name} = "{icon_filename(name)}"')
    lines.extend(("", "    var svgSource: String {", "        switch self {"))
    for name in manifest["icons"]:
        svg = render_icon(manifest, name).strip()
        lines.append(f'        case .{name}: #"{svg}"#')
    lines.extend(("        }", "    }", "}", ""))
    return "\n".join(lines)


class GeneratorTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tokens = json.loads(TOKENS.read_text(encoding="utf-8"))

    def test_canonical_palette_and_aliases_are_valid(self) -> None:
        validate(self.tokens)
        self.assertEqual(self.tokens["aliases"]["paper"], "surface")
        self.assertEqual(self.tokens["aliases"]["action"], "button")

    def test_unreadable_secondary_text_is_rejected(self) -> None:
        invalid = copy.deepcopy(self.tokens)
        invalid["colors"]["muted"]["light"] = "#AAAAAA"
        with self.assertRaisesRegex(ValueError, "contrast"):
            validate(invalid)

    def test_translucent_navigation_remains_readable_on_extreme_backdrops(self) -> None:
        for appearance in ("light", "dark"):
            for backdrop in ("#000000", "#FFFFFF"):
                backing = composite(self.tokens["colors"]["navigation-backing"][appearance], backdrop)
                self.assertGreaterEqual(contrast(self.tokens["colors"]["foreground"][appearance], backing), 4.5)
                self.assertGreaterEqual(contrast(self.tokens["colors"]["navigation-muted"][appearance], backing), 4.5)
        invalid = copy.deepcopy(self.tokens)
        invalid["colors"]["navigation-backing"]["light"] = "#F7F4EE00"
        with self.assertRaisesRegex(ValueError, "navigation text"):
            validate(invalid)

    def test_invalid_colors_and_aliases_are_rejected(self) -> None:
        invalid = copy.deepcopy(self.tokens)
        invalid["colors"]["surface"]["light"] = "white"
        with self.assertRaisesRegex(ValueError, "hex color"):
            validate(invalid)
        invalid = copy.deepcopy(self.tokens)
        invalid["aliases"]["paper"] = "missing"
        with self.assertRaisesRegex(ValueError, "alias"):
            validate(invalid)

    def test_pill_cannot_invert_with_the_window(self) -> None:
        invalid = copy.deepcopy(self.tokens)
        invalid["colors"]["pill-background"]["dark"] = "#202022"
        with self.assertRaisesRegex(ValueError, "remain stable"):
            validate(invalid)

    def test_generated_platforms_share_semantic_values(self) -> None:
        css = render_css(self.tokens)
        swift = render_swift(self.tokens)
        self.assertIn("--surface: #fcfbf8;", css)
        self.assertIn("--surface: #1e1c1a;", css)
        self.assertIn("surface = ColorPair(light: 0xFCFBF8, dark: 0x1E1C1A)", swift)
        self.assertEqual(css.count("--pill-background: #2a2723;"), 4)
        self.assertIn("lightAlpha: 0.10196078", swift)

    def test_interface_font_follows_the_original_figtree_and_can_use_the_system_font(self) -> None:
        css = render_css(self.tokens)
        self.assertIn('--ui: "Figtree", -apple-system', css)
        self.assertIn('--brand-ui: "Figtree"', css)
        self.assertIn("usesSystemInterface = false", render_swift(self.tokens))
        system = copy.deepcopy(self.tokens)
        system["typography"]["interface"] = "system"
        self.assertIn('--ui: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui', render_css(system))
        invalid = copy.deepcopy(self.tokens)
        invalid["typography"]["interface"] = "comic"
        with self.assertRaisesRegex(ValueError, "interface"):
            validate(invalid)

    def test_pill_geometry_is_shared_by_css_swift_and_frontend_state(self) -> None:
        css, swift, javascript = render_css(self.tokens), render_swift(self.tokens), render_pill_metrics(self.tokens)
        for name, value in self.tokens["pill"].items():
            unit = "" if name in PILL_COUNTS else "px"
            kind = "Int" if name in PILL_COUNTS else "CGFloat"
            self.assertIn(f"--pill-{name}: {scalar(value)}{unit};", css)
            self.assertIn(f"{swift_name(name)}: {kind} = {scalar(value)}", swift)
            self.assertIn(f"{swift_name(name)}: {scalar(value)},", javascript)

    def test_desktop_chrome_keeps_reading_fonts_and_shared_sidebar_bounds(self) -> None:
        css, swift = render_css(self.tokens), render_swift(self.tokens)
        self.assertIn('--ui: "Figtree"', css)
        self.assertIn(':root[data-platform="macos"] {\n  --chrome-ui: -apple-system', css)
        for name, value in self.tokens["desktop"].items():
            self.assertIn(f"--desktop-{name}: {scalar(value)}px", css)
            self.assertIn(f"{swift_name(name)}: CGFloat = {scalar(value)}", swift)
        invalid = copy.deepcopy(self.tokens)
        invalid["desktop"]["sidebar-width"] = invalid["desktop"]["sidebar-max-width"] + 1
        with self.assertRaisesRegex(ValueError, "within its bounds"):
            validate(invalid)
        invalid = copy.deepcopy(self.tokens)
        invalid["pill"]["preview-lines"] = 1.5
        with self.assertRaisesRegex(ValueError, "pill count"):
            validate(invalid)

    def test_system_and_forced_appearances_support_minimum_webkit(self) -> None:
        css = render_css(self.tokens)
        self.assertNotIn("light-dark(", css)
        media = css.index("@media (prefers-color-scheme: dark)")
        light = css.index(':root[data-appearance="light"]')
        dark = css.index(':root[data-appearance="dark"]')
        self.assertLess(media, light)
        self.assertLess(light, dark)
        system_block = css[media:light]
        light_block = css[light:dark]
        dark_block = css[dark:]
        for name, pair in self.tokens["colors"].items():
            self.assertIn(f"--{name}: {pair['light'].lower()};", css[:media])
            self.assertIn(f"--{name}: {pair['dark'].lower()};", system_block)
            self.assertIn(f"--{name}: {pair['light'].lower()};", light_block)
            self.assertIn(f"--{name}: {pair['dark'].lower()};", dark_block)

    def test_icon_shapes_are_shared_and_resource_safe(self) -> None:
        manifest = json.loads(ICON_MANIFEST.read_text(encoding="utf-8"))
        validate_icons(manifest)
        svg = render_icon(manifest, "arrowRight").strip()
        swift = render_swift_icons(manifest)
        self.assertEqual(icon_filename("arrowRight"), "arrow-right")
        self.assertIn(svg, swift)
        self.assertIn('stroke-width="1.7"', svg)
        invalid = copy.deepcopy(manifest)
        invalid["icons"]["mic"] = ['M0 0" onclick="bad']
        with self.assertRaisesRegex(ValueError, "SVG path"):
            validate_icons(invalid)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="validate tokens and verify generated files are current")
    mode.add_argument("--self-test", action="store_true", help="run generator and palette regression tests without writing files")
    args = parser.parse_args()
    if args.self_test:
        result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(GeneratorTests))
        return 0 if result.wasSuccessful() else 1
    try:
        tokens = json.loads(TOKENS.read_text(encoding="utf-8"))
        validate(tokens)
        manifest = json.loads(ICON_MANIFEST.read_text(encoding="utf-8"))
        validate_icons(manifest)
        outputs = [
            (CSS, render_css(tokens)),
            (SWIFT, render_swift(tokens)),
            (PILL_METRICS, render_pill_metrics(tokens)),
            (SWIFT_ICONS, render_swift_icons(manifest)),
            *((ICON_DIRECTORY / f"{icon_filename(name)}.svg", render_icon(manifest, name)) for name in manifest["icons"]),
        ]
        stale = []
        for path, content in outputs:
            if path.exists() and path.read_text(encoding="utf-8") == content:
                continue
            if args.check:
                stale.append(str(path.relative_to(ROOT)))
            else:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content, encoding="utf-8")
        if stale:
            print("Theme output is stale; run python3 scripts/generate-theme.py:", file=sys.stderr)
            print("\n".join(stale), file=sys.stderr)
            return 1
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"Theme generation failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
