# Language policy

Pennel uses English canonical defaults and application-owned diagnostics. The shared desktop and mobile interfaces support English and Dutch independently of speech recognition and user content. System language remains the default, with English fallback.

## Ownership

- Logs, errors before presentation, provider/model metadata and external tool scaffolding use English. Keep error codes stable and logs limited to existing timing, count and error metadata.
- Known app-owned display labels and failures use the selected interface language. Translate only owned fields, never arbitrary text, provider responses, account names, notes or transcripts.
- Unmodified built-in template names follow the interface. Core names and instructions are English; existing built-in IDs remain stable. Custom template names, explicit section headings and saved bodies remain literal.
- Default summary headings, generated prose and empty-section text follow the predominant meeting language. Ignore template wording, speaker markers and interface language when choosing it. Use notes if no transcript exists, then English if the language is indeterminate. Preserve user-note lines verbatim. Ordinary and long-summary prompts use the same policy.
- Consent and export scaffolding use the caller's resolved interface language. Existing entry points remain English-default wrappers.
- Cloud API/MCP diagnostics use English. Login emails follow the initiating app/site language. Website EN/NL/FR are supported; missing or unsupported mail locales fall back to English. German remains release-gated.

## Compatibility

`assets/locales/backend.sources.json` intentionally retains older Dutch message templates as compatibility aliases. English catalog templates and Dutch display templates are also recognized, only on owned error/status channels. Keep placeholder substitution literal and do not search/replace inside user content.

Transcript speaker markers (`Ik`, `Anderen`, numbered variants), failed-transcription markers, built-in template IDs and older audio filenames remain storage/protocol compatibility tokens. This cleanup does not rewrite source Markdown, migrate the search index or rename files. The newer recording layout already uses English audio filenames.

Core localized login initiation adds an optional request locale while retaining older entry points. Server support must ship before dependent app/site changes. Device approval links carry an explicit locale and retain fragment-only verification tokens. No stored account-language preference or database migration is required.

## Verification

Run `python3 -B -m unittest discover -s scripts/tests -p 'test_language_policy.py'` alongside core, desktop and native checks. It guards diagnostic producer literals; it deliberately excludes locale catalogs, test fixtures and frozen UI shells. Compatibility literals are narrowly listed in the test.

Regression coverage must distinguish UI language from meeting language, preserve custom content and legacy errors, and keep citations/indexing intact. Mail and billing checks use fake transports. Build/simulator evidence does not establish signed-device, deployment or store acceptance.
