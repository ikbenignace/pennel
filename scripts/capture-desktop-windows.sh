#!/usr/bin/env bash
# Captures the shared app's own windows (macOS only) for visual review.
# Runs the preview build with a temporary home and synthetic meetings, history and settings, and
# captures each Pennel window by its window id; the screen and other apps are never recorded.
# Build first: cd apps/desktop && npx tauri build --bundles app --config src-tauri/tauri.preview.conf.json
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
output="$root/build/visual-parity/windows"
appearances="light dark"
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output) output="$2"; shift 2 ;;
    --appearance) appearances="$2"; shift 2 ;;
    *) printf 'Unknown argument: %s\n' "$1" >&2; exit 2 ;;
  esac
done
[ "$(uname -s)" = Darwin ] || { echo "Window captures require macOS." >&2; exit 1; }
binary="$root/apps/desktop/src-tauri/target/release/bundle/macos/Pennel Preview.app/Contents/MacOS/pennel"
[ -x "$binary" ] || { echo "Build the preview app first." >&2; exit 1; }

work="$(mktemp -d "${TMPDIR:-/tmp}/pennel-windows-XXXXXX")"
home="$work/home"
data="$home/Library/Application Support/Pennel Preview"
pid=""
cleanup() { [ -n "$pid" ] && kill "$pid" 2>/dev/null; wait 2>/dev/null || true; rm -rf "$work"; }
trap cleanup EXIT
mkdir -p "$data/Meetings" "$output"

cat > "$work/windows.swift" <<'SWIFT'
import CoreGraphics
import Foundation
let pid = Int32(CommandLine.arguments[1])!
let title = CommandLine.arguments[2]
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
for window in windows where (window[kCGWindowOwnerPID as String] as? Int32) == pid
    && (window[kCGWindowName as String] as? String) == title && (window[kCGWindowLayer as String] as? Int) == 0 {
    print(window[kCGWindowNumber as String]!)
    break
}
SWIFT
swiftc -O "$work/windows.swift" -o "$work/windows" 2>/dev/null

meeting() {
  local id="$1" title="$2" start="$3" minutes="$4" summary="$5" transcript="$6"
  mkdir -p "$data/Meetings/$id"
  printf -- '---\ntitle: %s\nstart: %s\nduration_min: %s\ntemplate: algemeen\n---\n\n%s\n' "$title" "$start" "$minutes" "$summary" > "$data/Meetings/$id/meeting.md"
  printf '%s' "$transcript" > "$data/Meetings/$id/transcript.md"
  printf -- '- Planning\n- Volgende afspraak\n' > "$data/Meetings/$id/notes.md"
}
today="$(date +%Y-%m-%d)"
yesterday="$(date -v-1d +%Y-%m-%d)"
older="$(date -v-12d +%Y-%m-%d)"
offset="$(date +%z | sed 's/\(..\)$/:\1/')"
meeting "$today 0900 Weekstart" "Weekstart" "${today}T09:00:00$offset" 18 \
  $'## Samenvatting\n- Het team overloopt de planning van de week.\n- De release van vrijdag blijft staan.\n\n## Beslissingen\n- Noor neemt de klantdemo.\n\n## Actiepunten\n- [ ] Sam stuurt de agenda rond.' \
  $'[00:00] **Ik:** Goeiemorgen, zullen we de week overlopen?\n[00:06] **Anderen:** Ja, de release van vrijdag blijft staan.\n[00:15] **Ik:** Noor, neem jij de klantdemo?\n'
meeting "$yesterday 1400 Projectoverleg" "Projectoverleg" "${yesterday}T14:00:00$offset" 25 \
  $'## Samenvatting\n- De planning wordt maandag afgerond.\n- Het voorstel gaat deze week naar de klant.\n\n## Beslissingen\n- Geen extra scope voor de eerste versie.\n\n## Actiepunten\n- [ ] Volgende afspraak bevestigen.' \
  $'[00:00] **Anderen:** We ronden de planning maandag af.\n[00:08] **Ik:** Ik bevestig de volgende afspraak.\n[00:21] **Anderen:** Dan sturen we het voorstel deze week.\n'
meeting "$older 1030 Klantgesprek Studio Noord" "Klantgesprek Studio Noord" "${older}T10:30:00$offset" 42 \
  $'## Samenvatting\n- Studio Noord wil meetingnotities in het Nederlands.\n\n## Actiepunten\n- [ ] Offerte opmaken.' \
  $'[00:00] **Ik:** Fijn dat jullie tijd hebben.\n'

now_ms="$(($(date +%s) * 1000))"
sqlite3 "$data/history.sqlite" <<SQL
CREATE TABLE IF NOT EXISTS dictations (id TEXT PRIMARY KEY, created_at_ms INTEGER NOT NULL, app_bundle_id TEXT, raw TEXT NOT NULL, text TEXT NOT NULL, status TEXT NOT NULL, error TEXT, has_audio INTEGER NOT NULL);
PRAGMA user_version = 1;
INSERT INTO dictations VALUES ('qa-1', $((now_ms - 600000)), 'com.apple.MobileSMS', 'zullen we morgen om tien uur afspreken', 'Zullen we morgen om tien uur afspreken?', 'done', NULL, 0);
INSERT INTO dictations VALUES ('qa-2', $((now_ms - 1800000)), 'com.apple.mail', 'ik werk het voorstel bij', 'Ik werk het voorstel bij en stuur het morgen door.', 'done', NULL, 0);
INSERT INTO dictations VALUES ('qa-3', $((now_ms - 5400000)), 'com.apple.Notes', 'boodschappen', 'Boodschappen: koffie, melk en brood.', 'done', NULL, 0);
INSERT INTO dictations VALUES ('qa-4', $((now_ms - 90000000)), 'com.apple.Safari', 'zoek de openingsuren', 'Zoek de openingsuren van de bibliotheek.', 'done', NULL, 0);
INSERT INTO dictations VALUES ('qa-5', $((now_ms - 93000000)), 'com.apple.Terminal', 'git status', 'git status', 'done', NULL, 0);
SQL

# Each view gets a fresh process so its window renders the requested place on first load: WebKit can
# hold back responses to a web view that is occluded (as on a locked screen), so in-page navigation is
# not relied on.
launch() {
  stop
  HOME="$home" PENNEL_WINDOW_CAPTURE=1 "$binary" "$@" >>"$work/app.log" 2>&1 &
  pid=$!
}
stop() {
  if [ -n "$pid" ]; then kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true; pid=""; fi
}
run() { HOME="$home" PENNEL_WINDOW_CAPTURE=1 "$binary" "$@" >/dev/null 2>&1; }
wait_for() { for _ in $(seq 1 80); do id="$("$work/windows" "$pid" "$1")"; [ -n "$id" ] && return 0; sleep .25; done; return 1; }
settle() { sleep "${1:-1.5}"; }
save_settings() {
  launch
  wait_for "Welkom bij Pennel" || wait_for "Pennel"
  settle 2
  run --capture-eval main "(async()=>{const i=window.__TAURI_INTERNALS__;const s=(await i.invoke('app_info')).settings;$1;await i.invoke('save_settings',{settings:s});})()"
  settle 2
  stop
}
shoot() {
  local title="$1" name="$2" file="$output/$appearance/$2.png"
  wait_for "$title" || { echo "missing window: $title ($name)" >&2; return 1; }
  settle
  rm -f "$file"
  # screencapture sometimes writes the image but does not exit; give it a few seconds, then stop it.
  screencapture -x -o -l "$id" "$file" &
  local capture=$!
  for _ in $(seq 1 40); do kill -0 "$capture" 2>/dev/null || break; sleep .25; done
  kill "$capture" 2>/dev/null || true
  wait "$capture" 2>/dev/null || true
  [ -s "$file" ] || { echo "capture failed: $name" >&2; return 1; }
  echo "$appearance/$name.png"
}
view() {
  local title="$1" name="$2"; shift 2
  launch --capture-show "$@"
  shoot "$title" "$name"
}

rules="[['com.tinyspeck.slackmacgap','casual'],['net.whatsapp.WhatsApp','casual'],['com.apple.MobileSMS','casual'],['com.apple.mail','formal'],['com.microsoft.Word','formal'],['com.apple.Notes','neutral'],['com.apple.Terminal','literal'],['com.microsoft.VSCode','literal'],['dev.zed.Zed','literal']].map(([bundle_id,style])=>({bundle_id,style}))"
for appearance in $appearances; do
  mkdir -p "$output/$appearance"
  save_settings "s.appearance='$appearance';s.style_rules=$rules;s.dictionary=['Pennel','Studio Noord','Kubernetes'];s.onboarding.status='pending'"
  for step in welcome engine permissions practice; do
    view "Welkom bij Pennel" "onboarding-$step" onboarding "$step"
  done
  save_settings "s.onboarding.status='completed'"
  view "Pennel" "meeting-notes" library
  for tab in transcript ask; do
    run --capture-eval main "document.querySelector('[data-tab=\"$tab\"]')?.click()"
    shoot "Pennel" "meeting-$tab"
  done
  view "Pennel" "meetings-questions" library chat
  launch --capture-show library; wait_for "Pennel"; settle 2
  run --capture-eval main "document.querySelector('.meeting-row:last-child')?.dispatchEvent(new MouseEvent('contextmenu',{bubbles:true,clientX:120,clientY:330}))"
  shoot "Pennel" "meetings-context-menu"
  run --capture-eval main "document.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape'}));document.querySelector('.meeting-row:last-child .meeting-more')?.click();document.querySelectorAll('.context-menu button')[2]?.click()"
  shoot "Pennel" "dialog-delete-meeting"
  view "Pennel" "recording-start" live
  for tab in providers models dictation meeting-settings recording dictionary styles appearance account; do
    view "Instellingen" "settings-$tab" settings "$tab"
  done
  view "Geschiedenis" "history" history
  view "Dicteren" "dictation" dictation dictation-home
  view "Dicteren" "dictation-pad" dictation pad
done
stop
