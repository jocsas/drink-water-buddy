#!/usr/bin/env bash
# Capture a demo GIF of Hydrate Buddy's full reminder flow.
#
# Runs the app in dev mode (debug builds bypass active-hours, so the greeting
# nudge fires ~6s after launch), waits for the reminder window to appear,
# records just that window with ffmpeg x11grab, clicks the YES button with
# xdotool, and stops once the pet has walked out. The result is a looping,
# palette-optimized GIF written to docs/demo.gif.
#
# Requirements: ffmpeg, xdotool (gifsicle optional).
#
# Usage:
#   scripts/capture-demo.sh              # wizard theme (default)
#   THEME=default scripts/capture-demo.sh
#   SIZE=480 scripts/capture-demo.sh     # GIF width in px (default 720 = 2x)

set -euo pipefail

THEME="${THEME:-wizard}"
OUT="${OUT:-docs/demo.gif}"
SIZE="${SIZE:-720}"
FPS="${FPS:-12}"
LOGICAL_WIDTH=360
START_TIMEOUT_SECS=600     # generous: allows for a cold cargo build
CLICK_DELAY_SECS=2.6       # walk-in (1.15s) + a beat to read the bubble
HIDE_TIMEOUT_SECS=10       # max wait for the walk-out to complete
TAIL_SECS=0.3              # brief beat after the window hides (desktop shows through)
WORK="$(mktemp -d)"
RAW="$WORK/raw.mp4"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/com.jocsas.hydratebuddy/hydrate-buddy"
CONFIG_FILE="$CONFIG_DIR/config.json"
CONFIG_BACKUP="$WORK/config.json.bak"

APP_PID=""
FFMPEG_PID=""

log() { printf '\033[1;34m[capture]\033[0m %s\n' "$*"; }
die() { printf '\033[1;31m[capture]\033[0m %s\n' "$*" >&2; exit 1; }

cleanup() {
  if [ -n "${FFMPEG_PID:-}" ]; then
    kill -INT "$FFMPEG_PID" 2>/dev/null || true
  fi
  if [ -n "${APP_PID:-}" ]; then
    kill "$APP_PID" 2>/dev/null || true
    pkill -f 'target/debug/hydrate-buddy' 2>/dev/null || true
  fi
  if [ -f "$CONFIG_BACKUP" ]; then
    mkdir -p "$CONFIG_DIR"
    mv "$CONFIG_BACKUP" "$CONFIG_FILE"
  elif [ -f "$CONFIG_FILE" ]; then
    rm -f "$CONFIG_FILE" # we created it; there was none before
  fi
  rm -rf "$WORK"
}
trap cleanup EXIT

command -v ffmpeg >/dev/null || die "ffmpeg not found: sudo apt-get install -y ffmpeg"
command -v xdotool >/dev/null || die "xdotool not found: sudo apt-get install -y xdotool"

pgrep -f 'target/debug/hydrate-buddy' >/dev/null &&
  die "Hydrate Buddy is already running - quit it first (tray -> Quit)"

# --- config: force the theme, keep a backup ---------------------------------
mkdir -p "$CONFIG_DIR"
[ -f "$CONFIG_FILE" ] && cp "$CONFIG_FILE" "$CONFIG_BACKUP"
cat > "$CONFIG_FILE" <<EOF
{
  "name": "",
  "intervalMin": 45,
  "snoozeMin": 10,
  "themeId": "$THEME"
}
EOF
log "theme set to '$THEME'"

# --- launch dev build --------------------------------------------------------
log "starting tauri dev (a cold build can take a few minutes)..."
npm run tauri dev >"$WORK/dev.log" 2>&1 &
DEV_PID=$!

for _ in $(seq 1 "$START_TIMEOUT_SECS"); do
  APP_PID="$(pgrep -f 'target/debug/hydrate-buddy' | head -1 || true)"
  [ -n "$APP_PID" ] && break
  kill -0 "$DEV_PID" 2>/dev/null || { tail -50 "$WORK/dev.log"; die "tauri dev exited early"; }
  sleep 1
done
[ -n "$APP_PID" ] || { tail -50 "$WORK/dev.log"; die "app process never appeared"; }
log "app process $APP_PID up; waiting for the greeting nudge (~6s)..."

# --- wait for the reminder window to become visible --------------------------
WIN_ID=""
for _ in $(seq 1 $((START_TIMEOUT_SECS * 5))); do
  WIN_ID="$(xdotool search --onlyvisible --pid "$APP_PID" 2>/dev/null | head -1 || true)"
  [ -n "$WIN_ID" ] && break
  sleep 0.2
done
[ -n "$WIN_ID" ] || die "reminder window never became visible"

eval "$(xdotool getwindowgeometry --shell "$WIN_ID")" # sets X Y WIDTH HEIGHT
log "window $WIN_ID at ${X},${Y} size ${WIDTH}x${HEIGHT}"

SCALE="$(awk "BEGIN{print $WIDTH / $LOGICAL_WIDTH}")"

# --- record ------------------------------------------------------------------
log "recording..."
ffmpeg -y -loglevel error -f x11grab -framerate 15 -draw_mouse 0 \
  -video_size "${WIDTH}x${HEIGHT}" -i ":0.0+${X},${Y}" "$RAW" &
FFMPEG_PID=$!
sleep "$CLICK_DELAY_SECS"

# --- click YES (adaptive y: button shifts with prompt length) -----------------
click_at() { # logical_x logical_y
  xdotool mousemove --window "$WIN_ID" \
    "$(awk "BEGIN{printf \"%d\", $1 * $SCALE}")" \
    "$(awk "BEGIN{printf \"%d\", $2 * $SCALE}")"
  sleep 0.15
  xdotool click 1
}

confirmed=""
for cy in 102 124 82 145 62; do
  click_at 110 "$cy"
  # celebration + walk-out takes ~4s; poll until the window hides
  for _ in $(seq 1 $((HIDE_TIMEOUT_SECS * 5))); do
    if ! xdotool search --onlyvisible --pid "$APP_PID" 2>/dev/null | grep -q .; then
      confirmed=1
      break
    fi
    sleep 0.2
  done
  [ -n "$confirmed" ] && break
  log "no reaction at logical y=$cy, retrying..."
done
[ -n "$confirmed" ] || die "could not confirm the reminder (button never reacted)"

log "confirmed! letting the celebration finish..."
sleep "$TAIL_SECS"
kill -INT "$FFMPEG_PID" 2>/dev/null || true
wait "$FFMPEG_PID" 2>/dev/null || true
FFMPEG_PID=""

# --- palette-optimized GIF -----------------------------------------------------
mkdir -p "$(dirname "$OUT")"
log "building GIF (${SIZE}px wide, ${FPS}fps)..."
ffmpeg -y -loglevel error -i "$RAW" -vf \
  "fps=$FPS,scale=$SIZE:-1:flags=neighbor,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle" \
  -loop 0 "$OUT"
if command -v gifsicle >/dev/null; then
  gifsicle -O3 --colors 128 -i "$OUT" || true
fi

log "wrote $OUT ($(du -h "$OUT" | cut -f1))"
