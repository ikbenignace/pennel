# Desktop responsiveness measurements

The shared desktop performance work is tracked in [#49](https://github.com/pennel-app/pennel/issues/49). The real sixty-minute recording acceptance remains in [#12](https://github.com/pennel-app/pennel/issues/12).

## Owned WebKit comparison

`scripts/native-performance/run.mjs` builds an optimized frontend with the existing synthetic platform fixtures and loads it in an ad-hoc signed macOS application containing a real `WKWebView`. The helper has a nonpersistent website-data store, an owned loopback server and a dedicated bundle identifier. It does not open the installed Pennel application, inspect its files or credentials, record audio, contact a provider, or capture the user's screen.

This is **a packaged WebKit frontend benchmark, not a packaged Tauri product acceptance test**. The temporary frontend explicitly enables the fixture adapter at build time; ordinary release builds retain their existing fixture exclusion. Native Tauri popup menus, Rust command execution, audio/model processing and physical input are outside this harness. The menu adapter uses its deterministic renderer fixture path here.

Both scenarios use 1,000 meetings and a 2,400-segment, two-hour transcript. Fixed synthetic delays are applied to meeting detail (20 ms), voices, speaker setup, templates and summary annotations (40 ms each). These reproduce request ordering; they are not measured provider or filesystem latency. Response-byte accounting measures serialized synthetic data.

Run from the repository root on Mac with the existing desktop dependencies installed:

```sh
node scripts/native-performance/run.mjs \
  --source /tmp/pennel-native-baseline-8f4c8a1 \
  --output build/native-performance/baseline \
  --label 8f4c8a1

node scripts/native-performance/run.mjs \
  --output build/native-performance/candidate \
  --label candidate
```

The baseline directory must contain a frozen source export. For another comparison, export the chosen commit into an owned temporary directory with `git archive`; do not copy an installed app's data. When an exported source has no `node_modules`, the runner links it to this checkout's existing dependency installation. It installs no packages.

Each output contains an optimized frontend, the signed helper app, `library.json`, `recording.json`, logs, and `receipt.json`. The receipt records the source revision/label, frontend and helper SHA-256 fingerprints, environment, fixture sizes and measurement limits. These instrumented artifacts are for local QA and must not be distributed as Pennel releases.

## Scenarios and interpretation

- The library scenario opens six meetings, measures feedback and content readiness, changes one library row, opens a long transcript, scrolls it, edits notes and observes idle Settings for 6.5 seconds.
- The recording scenario observes 3.5 seconds without transcript changes, appends one segment, and checks that existing row identity and note focus survive.
- Transcript-tab synchronous work and the delay until the first painted frame are measured separately from sustained scripted scrolling. Frame gaps come from `requestAnimationFrame`; they do not establish native frame presentation or physical trackpad smoothness.
- CPU time and resident-memory observations cover only the owned helper PID. WebContent and GPU services cannot be reliably attributed by this harness and are explicitly excluded. Do not present these as total Pennel resource usage.

Initial targets are feedback under 100 ms, investigation of routine work exceeding 50 ms, and frame pacing appropriate to the actual refresh rate. Synthetic input includes waiting for painted frames; command latency, first materialization of large documents and ongoing scrolling must be assessed separately.

## Comparison recorded on 28 October 2026

The frozen baseline is `8f4c8a1`; the candidate includes the uncommitted native-desktop implementation on that base. Both use the same final harness. Machine: Mac16,8 with 48 GiB RAM, macOS 27.0.1 build 26A434, 980 × 640 logical-point window, 2× backing scale. The display advertises 120 Hz; this helper's observed animation callbacks ran near 60 Hz. These measurements do not certify 120 Hz presentation.

| Measurement | Baseline | Candidate |
| --- | ---: | ---: |
| Navigation feedback, p95 of six actions | 157 ms | 34 ms |
| Meeting content ready, p95 of six actions | 188 ms | 68 ms |
| Initial transcript-tab synchronous work | 21 ms | 24 ms |
| Initial transcript-tab to painted frames | 133 ms | 141 ms |
| Sustained scripted scroll, p95 / maximum frame gap | 18 / 21 ms | 17 / 18 ms |
| Scripted typing to painted frames, p95 of twenty edits | 34 ms | 35 ms |
| Selected content refresh overlapping a tab switch, synchronous / paint | 18 / 374 ms | 2 / 301 ms |
| Unchanged library row survives metadata change | No | Yes |
| Unrelated library metadata preserves the selected document | No | Yes |
| Unchanged transcript row survives a new segment | No | Yes |
| Live note focus survives update | Yes | Yes |

During 6.5 seconds in Settings, the baseline requested the full meeting list three times and full status three times (454,962 serialized response bytes). The candidate made three lightweight runtime-status calls (1,122 bytes), with no list or full-status request. During 3.5 seconds recording without new segments, the baseline requested four full statuses and one meeting list (1,859,617 bytes, plus one configuration response); the candidate made four lightweight status calls (2,112 bytes), with no list or transcript payload.

The initial two-hour transcript still exceeds the 100 ms first-paint target, and a genuine selected-document refresh overlapping a tab switch still takes 301 ms to paint. Separating synchronous construction from the following frames located the larger delay in first materialization/rendering rather than sustained scrolling. A helper-only `content-visibility` containment trial did not materially improve the first-paint result, so no containment styling was added. These costs remain explicit; the comparison establishes faster primary navigation and less recurring data work, not universal smoothness.

Host-only idle CPU was 0.359% → 0.039% in Settings and 1.301% → 0.230% during recording; sampled host idle RSS was 117,472 → 113,184 KiB and 87,200 → 82,816 KiB respectively. These short samples exclude WebContent/GPU and are not total application, energy, or sustained-recording measurements. Raw phase timings and samples are in the receipts under `build/native-performance/baseline/` and `build/native-performance/candidate/`.

Artifact fingerprints for this comparison:

| Artifact | SHA-256 |
| --- | --- |
| Same native helper executable in both runs | `593214bc48098a9c1aa0a4536ef4bdf83383f64f718c51a658fefbf5c99ad15f` |
| Frozen baseline optimized frontend | `1cbee4a6e7be01057bfee2a5af76338838889b2cfadf31d54bd487cf212bf2d2` |
| Candidate optimized frontend | `4943795b6d7dbac8a2ac2bb357248a2adf85a3f6996d31674a3fb33f3cb79e3d` |

Both scenarios completed with no unknown fixture commands or reported JavaScript errors. JavaScript syntax checks and the optimized Swift helper compilation passed. The helper was signed ad hoc and run from its `.app` bundle; production signing/notarization is separate.

## Product acceptance still required

Use the exact release Tauri artifact with owned synthetic files to measure native command timings, aggregate application/WebContent/GPU CPU and memory, idle energy, and model-loaded versus unloaded behavior. Inspect WebKit rendering timelines and native main-thread stacks for the same navigation, scrolling, typing and recording scenarios. Record the commit, package fingerprint, hardware, operating system and selected processing configuration.

Native menus require actual Tauri checks for selection/cancellation, disabled items, keyboard invocation, Retina/enlarged-text placement and VoiceOver. Physical keyboard/trackpad and Mac human acceptance, Windows/Linux native rendering, real audio, and the real sixty-minute call retain their own acceptance gates. Browser or helper results do not complete those gates.
