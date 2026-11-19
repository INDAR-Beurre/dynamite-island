#!/usr/bin/env bash
# capture.sh <outdir> <seconds> [fps] [options]
#
# Grabs numbered stills of the screen with `grim` in a tight loop. There is no
# screen recorder on this box, so this is the capture path: grim is a
# single-frame screenshot tool, NOT a recorder. Consequence, stated up front:
#
#   grim captures are not vsync-locked and each call costs ~30-95 ms, so the
#   nominal --fps is a ceiling, not a guarantee, and the wall-clock spacing
#   between frames is only good to a few ms. Absolute frame timing is
#   APPROXIMATE. What is still sound: the NUMBER of frames captured, the
#   number of DISTINCT (non-duplicate) frames, and the inter-frame visual
#   delta. Those reflect what reached the screen, not how fast grim ran.
#
# Options:
#   --region x0,y0,x1,y1   crop in LOGICAL px (grim -g's own space; the PNG/PPM
#                          comes back at scale 1.5, so 1764x200 -> 2646x300).
#                          default: 164,0,1340,200 (the island stage band ==
#                          STAGE_W 1176 x 200). The full screen is 1504x1002
#                          logical / 2256x1504 device.
#   --trigger "CMD@SEC"    send a control-socket command SEC after start
#                          (delegates to animate.py; safe commands only)
#   --format ppm|png       default ppm (grim writes PPM ~4x faster than PNG)
#
# Output: frame_0000.ppm ... plus timestamps.txt and meta.txt.
# timestamps.txt lines: <idx> <grim_start_ns> <grim_end_ns> <exit_code>
#   start/end are `date +%s%N` taken around each grim call, so the grim
#   duration per frame is directly visible and the inter-capture interval is
#   measured at both ends (start->start is the honest one).

set -u

OUT=${1:?usage: capture.sh <outdir> <seconds> [fps] [--region x0,y0,x1,y1]}
SECS=${2:-5}
FPS=${3:-20}
case "$FPS" in ''|*[!0-9]*) echo "fps must be a positive integer" >&2; exit 2;; esac
[ "$FPS" -gt 0 ] || { echo "fps must be > 0" >&2; exit 2; }
# seconds may be fractional (2.5); all timing math is done in integer ms
case "$SECS" in ''|*[!0-9.]*) echo "seconds must be a number" >&2; exit 2;; esac
SECS_MS=$(awk -v s="$SECS" 'BEGIN{printf "%d", s*1000}')
[ "$SECS_MS" -gt 0 ] || { echo "seconds must be > 0" >&2; exit 2; }

# Default region is in LAYOUT space. The output origin is not fixed (eDP-1
# moved to y=1543 mid-run), so query it rather than assuming 0,0.
ORIGIN=$(mmsg get all-monitors 2>/dev/null | python3 -c "
import json,sys
try:
    m=json.load(sys.stdin)['monitors']
    a=next((x for x in m if x.get('active')), m[0])
    print(a['x'], a['y'])
except Exception:
    print(0, 0)
" 2>/dev/null || echo "0 0")
read -r OX OY <<< "$ORIGIN"
REGION="$((OX+164)),$OY,$((OX+1340)),$((OY+200))"   # x0,y0,x1,y1, LOGICAL px
FORMAT=ppm
TRIGGER=""

shift 3 2>/dev/null || shift $#
while [ $# -gt 0 ]; do
  case "$1" in
    --region) REGION=${2:?}; shift 2;;
    --format) FORMAT=${2:?}; shift 2;;
    --trigger) TRIGGER=${2:?}; shift 2;;
    *) echo "unknown arg: $1" >&2; exit 2;;
  esac
done

IFS=, read -r X0 Y0 X1 Y1 <<< "$REGION"
[ -n "${X1:-}" ] && [ -n "${Y1:-}" ] || { echo "bad --region" >&2; exit 2; }
W=$((X1 - X0)); H=$((Y1 - Y0))
[ "$W" -gt 0 ] && [ "$H" -gt 0 ] || { echo "bad --region (x1<=x0 or y1<=y0)" >&2; exit 2; }
GEO="$X0,$Y0 ${W}x${H}"

command -v grim >/dev/null || { echo "grim not installed" >&2; exit 2; }

mkdir -p "$OUT" || exit 2
rm -f "$OUT"/frame_*.* "$OUT"/timestamps.txt "$OUT"/errors.log
: > "$OUT/timestamps.txt"

HERE=$(cd "$(dirname "$0")" && pwd)
if [ -n "$TRIGGER" ]; then
  TCMD=${TRIGGER%@*}; TSEC=${TRIGGER##*@}
  # Log the trigger time ONLY if the send actually landed. A socket that is
  # missing or refusing datagrams must not leave behind a window annotation
  # that makes the morph look measured when nothing was triggered.
  ( sleep "$TSEC"
    if python3 "$HERE/animate.py" "$TCMD" >>"$OUT/trigger.log" 2>&1; then
      echo "trigger $TCMD at $(date +%s%N)" >>"$OUT/trigger.log"
    else
      echo "trigger $TCMD FAILED rc=$? at $(date +%s%N)" >>"$OUT/trigger.log"
    fi ) &
fi

START=$(date +%s%N)
LOAD0=$(cat /proc/loadavg)
END=$(( START + SECS_MS * 1000000 ))
PERIOD_NS=$(( 1000000000 / FPS ))

i=0
while :; do
  NOW=$(date +%s%N)
  [ "$NOW" -ge "$END" ] && break

  # Pace only when actually ahead of schedule; never "catch up" by
  # firing a burst (a burst would fake a lower frame interval).
  TARGET=$(( START + i * PERIOD_NS ))
  AHEAD=$(( TARGET - NOW ))
  if [ "$AHEAD" -gt 4000000 ]; then
    sleep "$(printf '%d.%03d' $(( AHEAD / 1000000000 )) $(( (AHEAD / 1000000) % 1000 )))"
  fi

  FILE=$(printf '%s/frame_%04d.%s' "$OUT" "$i" "$FORMAT")
  TS0=$(date +%s%N)
  grim -g "$GEO" -t "$FORMAT" "$FILE" 2>>"$OUT/errors.log"
  RC=$?
  TS1=$(date +%s%N)
  echo "$i $TS0 $TS1 $RC" >> "$OUT/timestamps.txt"
  i=$((i + 1))
done
FINISH=$(date +%s%N)

CAPTURED=$(ls "$OUT"/frame_*."$FORMAT" 2>/dev/null | wc -l)

{
  echo "outdir=$OUT"
  echo "requested_seconds=$SECS"
  echo "requested_ms=$SECS_MS"
  echo "requested_fps=$FPS"
  echo "region=$REGION"
  echo "region_space=logical_px"
  # grim -g already cropped these frames to the region. Without this flag
  # analyze.py cannot know, and a caller passing --region as well would
  # crop a second time and slice the wrong pixels.
  echo "crop_applied_by_recorder=1"
  echo "logical_width=1504"
  echo "logical_height=1002"
  echo "geometry=$GEO"
  echo "format=$FORMAT"
  echo "frames_captured=$CAPTURED"
  echo "start_ns=$START"
  echo "finish_ns=$FINISH"
  echo "wall_seconds=$(printf '%d.%03d' $(( (FINISH - START) / 1000000000 )) $(( ((FINISH - START) / 1000000) % 1000 )))"
  echo "grim_version=$(grim -h 2>&1 | head -1)"
  echo "host=$(uname -n)"
  echo "date=$(date -Is)"
  echo "note=grim captures are not vsync-locked; intervals approximate to a few ms"
  echo "trigger=$TRIGGER"
  echo "loadavg_start=$LOAD0"
  echo "loadavg_end=$(cat /proc/loadavg)"
  echo "nproc=$(nproc)"
} > "$OUT/meta.txt"

echo "captured $CAPTURED frames into $OUT (requested ~$(( SECS_MS * FPS / 1000 )) at ${FPS}fps)"
