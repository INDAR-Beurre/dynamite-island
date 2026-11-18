#!/usr/bin/env bash
# record.sh <outdir> <seconds> [fps] [options]
#
# A REAL, timestamped screen recording of one region, split into still frames.
# This is the path that makes frame PACING measurable: wf-recorder pulls frames
# from the compositor's screencopy protocol and muxes them with the
# compositor's own presentation timestamps (PTS), so `dt` between frames is the
# screen's clock, not ours.
#
#   mode=cfr (default): -D -r <fps> asks for a frame on EVERY display slot
#     (no damage gating) at a constant <fps>. With a 60 Hz panel and -r 60 the
#     recording samples each of the 60 slots per second exactly once, so a
#     frame identical to its predecessor means the screen carried no new
#     content in that slot — a stalled redraw, not a quiet capture loop.
#     Caveat: forcing captures keeps the compositor awake, so this can only
#     FLATTER the panel. dt is a fixed 16.7 ms grid here: read pacing from the
#     duplicate/jump rates, not from dt.
#   mode=vfr (--vfr): no -D, no -r. Frames arrive only when the compositor has
#     damage, with true one-off PTS. dt is now honest and variable, which is
#     what you want for a LONG sustained animation; but an idle screen delivers
#     ~15 frames/s, so a 160 ms morph is sampled 2-3 times and cannot be
#     judged. Use --vfr for sustained motion only.
#
# Output contract is capture.sh's, plus the video, so analyze.py reads both:
#   frame_0000.ppm ...      frames, one per recorded frame (bit-exact: ffv1)
#   timestamps.txt          <idx> <pts_ns> <pts_ns+frame_duration> <rc>
#   meta.txt                same keys as capture.sh + capture_clock=compositor_pts
#   video.mkv               the recording itself (lossless ffv1, bgr0)
#   recorder.log            wf-recorder stderr
#
# Options:
#   --region x0,y0,x1,y1   crop in LOGICAL px (same space as capture.sh)
#                          default 600,0,900,150 (the island band)
#   --trigger "CMD@SEC"    send a safe control command SEC after start
#   --vfr                  damage-gated variable-rate capture (see above)
#
# Frames are ~300 KB each: a 4 s @ 60 fps run is ~250 MB. Point <outdir> at
# /tmp. Nothing here may be committed.

set -u

OUT=${1:?usage: record.sh <outdir> <seconds> [fps] [--region x0,y0,x1,y1] [--trigger CMD@SEC] [--vfr]}
SECS=${2:-3}
FPS=${3:-60}
case "$SECS" in ''|*[!0-9.]*) echo "seconds must be a number" >&2; exit 2;; esac
case "$FPS" in ''|*[!0-9]*) echo "fps must be a positive integer" >&2; exit 2;; esac

REGION="600,0,900,150"
TRIGGER=""
MODE=cfr
RESET=1

shift 3 2>/dev/null || shift $#
while [ $# -gt 0 ]; do
  case "$1" in
    --region) REGION=${2:?}; shift 2;;
    --trigger) TRIGGER=${2:?}; shift 2;;
    --vfr) MODE=vfr; shift;;
    --no-reset) RESET=0; shift;;
    *) echo "unknown arg: $1" >&2; exit 2;;
  esac
done

IFS=, read -r X0 Y0 X1 Y1 <<< "$REGION"
W=$((X1 - X0)); H=$((Y1 - Y0))
{ [ "$W" -gt 0 ] && [ "$H" -gt 0 ]; } || { echo "bad --region" >&2; exit 2; }
GEO="$X0,$Y0 ${W}x${H}"

command -v wf-recorder >/dev/null || { echo "wf-recorder not installed" >&2; exit 2; }
command -v ffmpeg >/dev/null || { echo "ffmpeg not installed" >&2; exit 2; }

mkdir -p "$OUT" || exit 2
rm -f "$OUT"/frame_*.ppm "$OUT"/video.mkv "$OUT"/timestamps.txt "$OUT"/trigger.log

HERE=$(cd "$(dirname "$0")" && pwd)

# Start from a known state. Without this a repeated capture of `music -> music`
# measures nothing: the panel is already there, nothing morphs, and the run
# reads as a 0-pose stall that is really a no-op trigger.
if [ "$RESET" = 1 ]; then
  python3 "$HERE/animate.py" collapsed >/dev/null 2>&1 && sleep 0.6
fi
if [ -n "$TRIGGER" ]; then
  TCMD=${TRIGGER%@*}; TSEC=${TRIGGER##*@}
  # Log the trigger time only if the send landed (a missing socket must not
  # leave a window annotation behind that implies a morph was measured).
  ( sleep "$TSEC"
    if python3 "$HERE/animate.py" "$TCMD" >>"$OUT/trigger.log" 2>&1; then
      echo "trigger $TCMD at $(date +%s%N)" >>"$OUT/trigger.log"
    else
      echo "trigger $TCMD FAILED rc=$? at $(date +%s%N)" >>"$OUT/trigger.log"
    fi ) &
fi

RFLAGS=(-g "$GEO" -c ffv1 -f "$OUT/video.mkv")
[ "$MODE" = cfr ] && RFLAGS+=(-D -r "$FPS")

START=$(date +%s%N)
LOAD0=$(cat /proc/loadavg)
wf-recorder "${RFLAGS[@]}" 2>"$OUT/recorder.log" &
RPID=$!
sleep "$SECS"
# SIGINT, not SIGTERM: the muxer needs it to write the container index.
kill -INT "$RPID" 2>/dev/null
wait "$RPID" 2>/dev/null
FINISH=$(date +%s%N)

[ -s "$OUT/video.mkv" ] || { echo "recording produced no file; see $OUT/recorder.log" >&2; exit 3; }

# Split the recording into stills. fps_mode passthrough = keep every decoded
# frame, no resampling, so frame i maps 1:1 onto PTS i.
if ! ffmpeg -v error -y -i "$OUT/video.mkv" -fps_mode passthrough \
     -pix_fmt rgb24 -start_number 0 "$OUT/frame_%04d.ppm" 2>"$OUT/extract.log"; then
  # ffmpeg < 5 has no -fps_mode.
  ffmpeg -v error -y -i "$OUT/video.mkv" -vsync 0 \
     -pix_fmt rgb24 -start_number 0 "$OUT/frame_%04d.ppm" 2>>"$OUT/extract.log" \
     || { echo "frame extraction failed; see $OUT/extract.log" >&2; exit 4; }
fi

# timestamps.txt from the container's own PTS. end_ns = pts + the following
# frame's interval (last frame uses the median interval), so analyze.py's
# existing start/end contract works unchanged.
python3 - "$OUT" "$START" <<'PY' || exit 5
import statistics, subprocess, sys
from pathlib import Path

out = Path(sys.argv[1])
t0 = int(sys.argv[2])   # wall-clock ns when wf-recorder was launched
def pts(p):
    r = subprocess.run(["ffprobe", "-v", "error", "-select_streams", "v:0",
                        "-show_entries", "frame=pts_time", "-of", "csv=p=0", str(p)],
                       capture_output=True, text=True)
    return [float(x.strip().rstrip(",")) for x in r.stdout.splitlines() if x.strip().rstrip(",")]

t = pts(out / "video.mkv")
frames = sorted(out.glob("frame_*.ppm"))
if not t:
    sys.exit("no PTS in recording")
if len(t) != len(frames):
    # Silent misalignment would attribute the wrong time to every frame.
    sys.exit(f"frame/PTS mismatch: {len(frames)} frames, {len(t)} PTS")
gap = [b - a for a, b in zip(t, t[1:])] or [1 / 60]
med = statistics.median(gap)
# PTS is relative to the capture start; rebase it onto the wall clock so
# timestamps.txt and trigger.log (date +%s%N) live in ONE clock domain.
# Only differences are used, but the trigger anchor needs the shared domain.
# Offset error = wf-recorder's own start-up, ~0.1 s; see docs/.
with open(out / "timestamps.txt", "w") as fh:
    for i, a in enumerate(t):
        dur = gap[i] if i < len(gap) else med
        fh.write(f"{i} {t0 + int(round(a*1e9))} "
                 f"{t0 + int(round((a+dur)*1e9))} 0\n")
print(f"pts: {len(t)} frames, span {t[-1]-t[0]:.3f} s, median dt {med*1000:.1f} ms")
PY

CAPTURED=$(ls "$OUT"/frame_*.ppm 2>/dev/null | wc -l)
{
  echo "outdir=$OUT"
  echo "recorder=wf-recorder"
  echo "recorder_version=$(wf-recorder --version 2>&1 | head -1)"
  echo "recorder_mode=$MODE"
  echo "capture_clock=compositor_pts"
  echo "crop_applied_by_recorder=1"
  echo "requested_seconds=$SECS"
  echo "requested_ms=$(awk -v s="$SECS" 'BEGIN{printf "%d", s*1000}')"
  echo "requested_fps=$FPS"
  echo "region=$REGION"
  echo "region_space=logical_px"
  echo "logical_width=1504"
  echo "logical_height=1002"
  echo "geometry=$GEO"
  echo "format=ppm"
  echo "codec=ffv1"
  echo "frames_captured=$CAPTURED"
  echo "start_ns=$START"
  echo "clock=pts_rebased_on_wall_clock"
  echo "finish_ns=$FINISH"
  echo "wall_seconds=$(awk -v a="$START" -v b="$FINISH" 'BEGIN{printf "%.3f", (b-a)/1e9}')"
  echo "host=$(uname -n)"
  echo "date=$(date -Is)"
  echo "note=wf-recorder PTS from the compositor; cfr mode samples every display slot"
  echo "trigger=$TRIGGER"
  echo "pre_state=$( [ "$RESET" = 1 ] && echo collapsed || echo untouched )"
  echo "loadavg_start=$LOAD0"
  echo "loadavg_end=$(cat /proc/loadavg)"
  echo "nproc=$(nproc)"
} > "$OUT/meta.txt"

echo "recorded $CAPTURED frames into $OUT"
