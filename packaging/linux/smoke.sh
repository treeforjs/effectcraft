#!/usr/bin/env bash
# Exercise installed binaries in a desktop session, or with xvfb-run + dbus-run-session.
# Overrides: EFFECTCRAFT_BIN, EFFECTCRAFT_CLI, SMOKE_OUT, EFFECTCRAFT_SMOKE_PORT.
set -euo pipefail
APP="${EFFECTCRAFT_BIN:-effectcraft}"
CLI="${EFFECTCRAFT_CLI:-effectcraft-cli}"
OUT="${SMOKE_OUT:-$(mktemp -d -t effectcraft-smoke.XXXXXX)}"
PORT="${EFFECTCRAFT_SMOKE_PORT:-19877}"
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
for file in input.ecproj output.png output.mp4 decoded.ecproj decoded.png imported.json original.json edited.ecproj reopened.json desktop.png desktop.log control.jsonl create.json config; do
  if [ -e "$OUT/$file" ] || [ -L "$OUT/$file" ]; then
    echo "error: smoke output already exists: $OUT/$file (use a fresh SMOKE_OUT directory)" >&2
    exit 1
  fi
done
command -v jq >/dev/null || { echo "error: jq is required" >&2; exit 1; }
"$APP" --version
"$CLI" --version
"$CLI" run comp.new '{"name":"Ubuntu smoke","width":128,"height":96,"duration":1,"frameRate":24}' \
  layer.newSolid '{"name":"Orange","color":[1,0.25,0.05],"width":128,"height":96}' \
  --empty --save-as "$OUT/input.ecproj" --json >"$OUT/create.json"
"$CLI" info --project "$OUT/input.ecproj" --json >"$OUT/original.json"
"$CLI" render-frame --project "$OUT/input.ecproj" --out "$OUT/output.png" --json
"$CLI" render --project "$OUT/input.ecproj" --out "$OUT/output.mp4" --format h264 --start 0 --end 0.125 --audio off --json
test -s "$OUT/output.png"
test -s "$OUT/output.mp4"
# Read our encoded video back through the real media importer and decoder.
"$CLI" exec file.import "$(jq -nc --arg path "$OUT/output.mp4" '{paths:[$path],addToComp:true}')" \
  --project "$OUT/input.ecproj" --save-as "$OUT/decoded.ecproj" --json >"$OUT/imported.json"
jq -e '(.result.errors | length) == 0 and (.result.items | length) == 1' "$OUT/imported.json" >/dev/null
"$CLI" render-frame --project "$OUT/decoded.ecproj" --out "$OUT/decoded.png" --json
test -s "$OUT/decoded.png"

: >"$OUT/desktop.log"
EFFECTCRAFT_CONFIG_DIR="$OUT/config" RUST_LOG=info "$APP" --control "$PORT" "$OUT/input.ecproj" >"$OUT/desktop.log" 2>&1 &
APP_PID=$!
cleanup() {
  kill "$APP_PID" 2>/dev/null || true
  wait "$APP_PID" 2>/dev/null || true
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
CONNECTED=0
for ((attempt = 0; attempt < 60; attempt++)); do
  if ! kill -0 "$APP_PID" 2>/dev/null; then cat "$OUT/desktop.log" >&2; exit 1; fi
  # A different instance may already own the port: only drive the process we just started.
  if grep -Fq "effectcraft: control server listening on 127.0.0.1:$PORT" "$OUT/desktop.log" \
    && { exec 3<>"/dev/tcp/127.0.0.1/$PORT"; } 2>/dev/null; then CONNECTED=1; break; fi
  sleep 0.5
done
if [ "$CONNECTED" != 1 ]; then cat "$OUT/desktop.log" >&2; echo "desktop did not start" >&2; exit 1; fi

rpc() {
  local reply
  printf '%s\n' "$1" >&3
  if ! IFS= read -r -t 30 reply <&3; then
    cat "$OUT/desktop.log" >&2
    echo "desktop control request timed out" >&2
    return 1
  fi
  printf '%s\n' "$reply" | tee -a "$OUT/control.jsonl" | jq -e '.ok == true' >/dev/null
}
rpc '{"id":1,"method":"ui.inspect","params":{}}'
jq -e 'select(.id == 1) | .result.activeComp != null' "$OUT/control.jsonl" >/dev/null
rpc '{"id":2,"method":"engine.execute","params":{"command":"layer.newSolid","params":{"name":"Saved blue layer","color":[0.1,0.3,0.8],"width":48,"height":32}}}'
rpc "$(jq -nc --arg path "$OUT/edited.ecproj" '{id:3,method:"engine.execute",params:{command:"file.saveAs",params:{path:$path}}}')"
for ((attempt = 0; attempt < 60; attempt++)); do
  if [ -s "$OUT/edited.ecproj" ]; then break; fi
  sleep 0.5
done
test -s "$OUT/edited.ecproj"
"$CLI" info --project "$OUT/edited.ecproj" --json >"$OUT/reopened.json"
jq -e --slurpfile original "$OUT/original.json" \
  '(.activeComp.layers | length) == (($original[0].activeComp.layers | length) + 1) and
   any(.activeComp.layers[]; .name == "Saved blue layer")' "$OUT/reopened.json" >/dev/null
rpc "$(jq -nc --arg path "$OUT/desktop.png" '{id:4,method:"ui.screenshot",params:{path:$path}}')"
test -s "$OUT/desktop.png"
echo "EffectCraft desktop, edit/save/reopen and CLI smoke passed: $OUT"
