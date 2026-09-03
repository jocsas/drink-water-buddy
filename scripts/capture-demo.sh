#!/usr/bin/env bash
# Capture a demo GIF of Hydrate Buddy's full reminder flow.
#
# Runs the app in dev mode (debug builds bypass active-hours, so the greeting
# nudge fires ~6s after launch), waits for the reminder window to appear,
# records just that window, confirms the reminder, and stops once the pet has
# walked out. Result: a looping 60fps MP4 written to docs/demo.mp4 (or a
# palettized GIF via OUT=docs/demo.gif).
#
# Works on Linux/X11 (ffmpeg x11grab + xdotool) and macOS (ffmpeg avfoundation
# + osascript System Events). On macOS, grant the terminal running this script
# Screen Recording permission (System Settings > Privacy & Security), run it on
# the main display, and quit any installed Hydrate Buddy first. macOS confirm
# needs no accessibility permission: HYDRATE_BUDDY_AUTOCONFIRM_MS makes the
# debug build click YES itself.
#
# Requirements: ffmpeg; on Linux also xdotool (gifsicle optional everywhere).
#
# Usage:
#   scripts/capture-demo.sh              # wizard theme (default)
#   THEME=default scripts/capture-demo.sh
#   SIZE=480 scripts/capture-demo.sh     # GIF width in px (default 720 = 2x)

set -euo pipefail

OS="$(uname)"
THEME="${THEME:-wizard}"
OUT="${OUT:-docs/demo.mp4}" # OUT=docs/demo.gif for a palettized loop instead
SIZE="${SIZE:-720}"
FPS="${FPS:-15}"            # GIF frame rate; must divide RECORD_FPS evenly
RECORD_FPS=60 # source rate; MP4 keeps it, GIF downsamples (60/15 = 4:1)
LOGICAL_WIDTH=360
START_TIMEOUT_SECS=600     # generous: allows for a cold cargo build
CLICK_DELAY_SECS=2.6       # walk-in (1.15s) + a beat to read the bubble
HIDE_TIMEOUT_SECS=10       # max wait for the walk-out to complete
TAIL_SECS=0.3              # brief beat after the window hides (desktop shows through)
WORK="$(mktemp -d)"
RAW="$WORK/raw.mp4"
if [ "$OS" = "Darwin" ]; then
  CONFIG_DIR="$HOME/Library/Application Support/com.jocsas.hydratebuddy"
else
  CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/com.jocsas.hydratebuddy"
fi
CONFIG_FILE="$CONFIG_DIR/hydrate-buddy/config.json"
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

command -v ffmpeg >/dev/null || die "ffmpeg not found (macOS: brew install ffmpeg; Linux: sudo apt-get install -y ffmpeg)"
if [ "$OS" = "Darwin" ]; then
  command -v osascript >/dev/null || die "osascript not found"
else
  command -v xdotool >/dev/null || die "xdotool not found: sudo apt-get install -y xdotool"
fi

pgrep -f 'target/debug/hydrate-buddy' >/dev/null &&
  die "Hydrate Buddy is already running - quit it first (tray -> Quit)"

# --- macOS helpers (window info & clicks via System Events) -------------------
mac_window_count() { # pid -> number of on-screen windows
  osascript -e "tell application \"System Events\" to count windows of (first application process whose unix id is $1)" 2>/dev/null || echo 0
}

mac_window_bounds() { # pid -> "X Y W H" in points
  osascript -e "tell application \"System Events\" to tell (first application process whose unix id is $1)
    get {position, size} of front window
  end tell" 2>/dev/null | tr -d ' ' | tr ',' ' '
}

mac_screen_points() { # -> "W H" of the main display, in points
  osascript -e 'tell application "Finder" to get bounds of window of desktop' \
    | tr -d ' ' | tr ',' ' ' | awk '{print $3 - $1, $4 - $2}'
}

av_screen_index() { # ffmpeg avfoundation device index of the first screen capture device
  # Note: -list_devices always exits nonzero after listing; tolerate it.
  local listing
  listing="$(ffmpeg -f avfoundation -list_devices true -i "" 2>&1 || true)"
  printf '%s\n' "$listing" | sed -n 's/.*\[\([0-9][0-9]*\)\] Capture screen.*/\1/p' | head -1 || true
}

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
if [ "$OS" = "Darwin" ]; then
  # Marker-file handshake (debug build): the app holds the reminder window
  # until recording frames flow, and clicks YES only once armed - so the
  # recorder always catches the full walk-in -> bubble -> click -> walk-out.
  DEFER_MARKER="$WORK/defer-show.marker"
  ARMED_MARKER="$WORK/autoconfirm-armed.marker"
  export HYDRATE_BUDDY_DEFER_SHOW="$DEFER_MARKER"
  export HYDRATE_BUDDY_AUTOCONFIRM_ARMED="$ARMED_MARKER"
  export HYDRATE_BUDDY_AUTOCONFIRM_MS="$(awk "BEGIN{printf \"%d\", $CLICK_DELAY_SECS * 1000}")"
fi
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
window_visible() { # -> 0 if the reminder window is on screen
  if [ "$OS" = "Darwin" ]; then
    [ "$(mac_window_count "$APP_PID")" -ge 1 ]
  else
    xdotool search --onlyvisible --pid "$APP_PID" 2>/dev/null | grep -q .
  fi
}

wait_for_window() {
  for _ in $(seq 1 $((START_TIMEOUT_SECS * 5))); do
    if window_visible; then
      sleep 0.5 # let the walk-in animation settle before grabbing geometry
      return 0
    fi
    kill -0 "$APP_PID" 2>/dev/null || die "app died"
    sleep 0.2
  done
  return 1
}

if [ "$OS" = "Darwin" ]; then
  # --- record first, then let the reminder happen -----------------------------
  SCREEN_IDX="$(av_screen_index)"
  [ -n "$SCREEN_IDX" ] || die "no avfoundation screen capture device found"
  log "recording full screen (the reminder is deferred until frames flow)..."
  ffmpeg -y -loglevel error -f avfoundation -framerate "$RECORD_FPS" -capture_cursor 0 \
    -pixel_format uyvy422 -i "$SCREEN_IDX" "$RAW" &
  FFMPEG_PID=$!

  raw_size() { stat -f %z "$RAW" 2>/dev/null || echo 0; }
  for _ in $(seq 1 100); do
    [ "$(raw_size)" -gt 100000 ] && break
    kill -0 "$FFMPEG_PID" 2>/dev/null || die "ffmpeg exited before producing frames"
    sleep 0.2
  done
  [ "$(raw_size)" -gt 100000 ] || die "recorder never wrote frames (Screen Recording permission?)"

  touch "$DEFER_MARKER"
  log "recorder rolling; releasing the reminder window..."

  wait_for_window || die "reminder window never became visible"
  read -r X Y WIDTH HEIGHT <<<"$(mac_window_bounds "$APP_PID")"
  [ -n "${WIDTH:-}" ] || die "could not read window geometry"
  read -r SCREEN_W_PT SCREEN_H_PT <<<"$(mac_screen_points)"
  SCALE=""
  log "window at ${X},${Y} size ${WIDTH}x${HEIGHT}"

  touch "$ARMED_MARKER"
  log "armed; the app clicks YES in ${CLICK_DELAY_SECS}s (walk-in + bubble)..."

  confirmed=""
  for _ in $(seq 1 $(((HIDE_TIMEOUT_SECS + 15) * 5))); do
    if ! window_visible; then
      confirmed=1
      break
    fi
    sleep 0.2
  done
  [ -n "$confirmed" ] || die "reminder window never hid (autoclick failed?)"
else
  wait_for_window || die "reminder window never became visible"
  WIN_ID="$(xdotool search --onlyvisible --pid "$APP_PID" | head -1)"
  eval "$(xdotool getwindowgeometry --shell "$WIN_ID")" # sets X Y WIDTH HEIGHT
  SCALE="$(awk "BEGIN{print $WIDTH / $LOGICAL_WIDTH}")"
  log "window at ${X},${Y} size ${WIDTH}x${HEIGHT}"

  # --- record ----------------------------------------------------------------
  log "recording..."
  ffmpeg -y -loglevel error -f x11grab -framerate "$RECORD_FPS" -draw_mouse 0 \
    -video_size "${WIDTH}x${HEIGHT}" -i ":0.0+${X},${Y}" "$RAW" &
  FFMPEG_PID=$!
  sleep "$CLICK_DELAY_SECS"

  # --- click YES (adaptive y: button shifts with prompt length) ---------------
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
      if ! window_visible; then
        confirmed=1
        break
      fi
      sleep 0.2
    done
    [ -n "$confirmed" ] && break
    log "no reaction at logical y=$cy, retrying..."
  done
  [ -n "$confirmed" ] || die "could not confirm the reminder (button never reacted)"
fi

log "confirmed! letting the celebration finish..."
sleep "$TAIL_SECS"
kill -INT "$FFMPEG_PID" 2>/dev/null || true
wait "$FFMPEG_PID" 2>/dev/null || true
FFMPEG_PID=""

# --- palette-optimized GIF -----------------------------------------------------
mkdir -p "$(dirname "$OUT")"
CROP_VF=""
TRIM_SECS=0
if [ "$OS" = "Darwin" ]; then
  # raw.mp4 is a full-screen grab (Retina = physical px); crop to the window.
  VID_W="$(ffprobe -v error -select_streams v:0 -show_entries stream=width -of csv=p=0 "$RAW")"
  PX_SCALE="$(awk "BEGIN{print $VID_W / $SCREEN_W_PT}")"
  CX="$(awk "BEGIN{printf \"%d\", int($X * $PX_SCALE)}")"
  CY="$(awk "BEGIN{printf \"%d\", int($Y * $PX_SCALE)}")"
  CW="$(awk "BEGIN{printf \"%d\", int($WIDTH * $PX_SCALE)}")"
  CH="$(awk "BEGIN{printf \"%d\", int($HEIGHT * $PX_SCALE)}")"
  CROP_EXPR="crop=${CW}:${CH}:${CX}:${CY}"
  CROP_VF="${CROP_EXPR},"
  log "cropping ${CW}x${CH}+${CX},${CY} (retina scale ${PX_SCALE})"

  # Trim the desktop-only head: first motion inside the crop region is the
  # walk-in. Keep a small lead-in so the GIF eases in.
  FIRST_MOTION="$(ffmpeg -i "$RAW" -an -vf "${CROP_EXPR},select='gt(scene,0.08)',showinfo" \
    -fps_mode vfr -f null - 2>&1 | sed -n 's/.*pts_time:\([0-9.]*\).*/\1/p' | head -1 || true)"
  if [ -n "$FIRST_MOTION" ]; then
    TRIM_SECS="$(awk "BEGIN{t=$FIRST_MOTION-0.4; if(t<0)t=0; printf \"%.2f\", t}")"
    log "first motion at ${FIRST_MOTION}s; trimming head to ${TRIM_SECS}s"
  else
    log "warning: no motion detected; keeping full recording"
  fi
fi
case "$OUT" in
  *.mp4)
    log "encoding MP4 (${SIZE}px wide, ${RECORD_FPS}fps)..."
    ffmpeg -y -loglevel error -ss "$TRIM_SECS" -i "$RAW" -vf \
      "${CROP_VF}scale=$SIZE:-1:flags=neighbor" \
      -c:v libx264 -crf 20 -preset slow -pix_fmt yuv420p \
      -movflags +faststart -r "$RECORD_FPS" "$OUT"
    ;;
  *)
    log "building GIF (${SIZE}px wide, ${FPS}fps)..."
    ffmpeg -y -loglevel error -ss "$TRIM_SECS" -i "$RAW" -vf \
      "fps=$FPS,${CROP_VF}scale=$SIZE:-1:flags=neighbor,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle" \
      -loop 0 "$OUT"
    if command -v gifsicle >/dev/null; then
      gifsicle -O3 --colors 128 -i "$OUT" || true
    fi
    ;;
esac

log "wrote $OUT ($(du -h "$OUT" | cut -f1))"
