# Pennel compact interface icons

The path geometry in `manifest.json` was authored directly for Pennel during the desktop redesign. It follows familiar 24 × 24 outline-icon conventions, with rounded caps and joins and a shared 1.7-unit stroke. The frontend implementation author reported that no third-party icon package or upstream SVG path source was fetched or copied during authoring.

Lucide and Feather were style references. Similar basic arrows, checks, rectangles and circles occur across icon sets; that similarity alone does not establish a specific upstream path source. These assets are therefore recorded as locally authored rather than labeled as unmodified Lucide or Feather icons. Project licensing is recorded in the repository's [LICENSE](../../../LICENSE).

The canonical manifest produces the SVG files and the embedded native SVG data through `scripts/generate-theme.py`. Keep geometry and names in that manifest so the shared and native interfaces render the same shapes.

Upstream references reviewed for provenance:

- [Lucide source](https://github.com/lucide-icons/lucide) and [ISC / Feather-derived MIT notices](https://github.com/lucide-icons/lucide/blob/main/LICENSE).
- [Feather source](https://github.com/feathericons/feather) and [MIT license](https://github.com/feathericons/feather/blob/main/LICENSE).

If an upstream glyph is imported or adapted later, record its exact source and preserve the applicable upstream copyright and permission notice with the asset.
