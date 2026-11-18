#!/usr/bin/env bash
# run_all.sh — the whole baseline in one command.
#
#   1. capture the settled collapsed pill for 5 s        (baseline: is it static?)
#   2. capture the same band while ONE transition runs   (baseline: the morph)
#   3. print a one-screen summary of both
#
# Everything lands in scripts/motion/out/<timestamp>/ so runs are diffable.
# Nothing here touches src/, cargo, or the panel process: state changes go out
# through the panel's own control socket via animate.py.

set -u
HERE=$(cd "$(dirname "$0")" && pwd)
STAMP=$(date +%Y%m%d-%H%M%S)
OUT=${1:-"$HERE/out/$STAMP"}

# Band that contains the pill in every state: collapsed 74dp wide up to the
# ~200dp expanded states, centred on the 1504-wide logical screen.
REGION="600,0,900,150"

SETTLE=5
FPS=20

mkdir -p "$OUT" || exit 2

# Park the panel in a known state before measuring. `collapsed` is a state word
# the handler morphs into; harmless and idempotent.
python3 "$HERE/animate.py" collapsed >/dev/null 2>&1
sleep 1

echo "#################### 1/2  SETTLED (collapsed, no animation) ####################"
bash "$HERE/capture.sh" "$OUT/settled" "$SETTLE" "$FPS" --region "$REGION" \
  | tail -1
python3 "$HERE/analyze.py" "$OUT/settled" > "$OUT/settled.txt" 2>&1
cat "$OUT/settled.txt"

echo
echo "#################### 2/2  TRANSITION (collapsed -> music) ####################"
# The morph is 160 ms (MORPH_DURATION_MS in src/morph.rs) and the panel redraws
# it on a 16 ms timer. Trigger lands 1.0 s into a 4 s capture so there is clean
# pre-roll and post-roll around the animation.
python3 "$HERE/animate.py" collapsed >/dev/null 2>&1
sleep 1
bash "$HERE/capture.sh" "$OUT/morph" 4 "$FPS" --region "$REGION" --trigger "music@1.0" \
  | tail -1
python3 "$HERE/analyze.py" "$OUT/morph" > "$OUT/morph.txt" 2>&1
cat "$OUT/morph.txt"

echo
echo "#################### 3/3  SUSTAINED (dictation bars driving continuously) ####################"
# The 160 ms morph is under-sampled at any rate grim can reach, so it cannot
# prove smoothness either way. Dictation bars keep redrawing for seconds, which
# makes frame spacing measurable. A Python driver pushes 27 bar amplitudes at
# ~100 Hz while the capture loop runs, so the panel must redraw on every frame.
python3 "$HERE/animate.py" collapsed >/dev/null 2>&1
sleep 0.5
python3 "$HERE/drive_bars.py" 6 > "$OUT/bars_driver.log" 2>&1 &
DRIVER=$!
sleep 1.5
bash "$HERE/capture.sh" "$OUT/sustained" 3 "$FPS" --region "$REGION" | tail -1
wait $DRIVER 2>/dev/null
python3 "$HERE/analyze.py" "$OUT/sustained" > "$OUT/sustained.txt" 2>&1
cat "$OUT/sustained.txt"

# Leave the panel where we found it.
python3 "$HERE/animate.py" collapsed >/dev/null 2>&1

# ---------------------------------------------------------------- summary ----
# Pull one already-computed field out of an analyze.py report by its line
# label. Field text is taken verbatim from that line, so the summary can never
# drift from the report it claims to summarise.
field() {  # field FILE LABEL REGEX
  local f=$1 label=$2 re=$3 line
  line=$(grep -m1 "^$label" "$f" 2>/dev/null) || { printf '%s' "-"; return; }
  printf '%s' "$(printf '%s' "$line" | grep -oE "$re" | head -1)"
}

S=$OUT/settled.txt
M=$OUT/morph.txt
U=$OUT/sustained.txt

{
echo
echo "================================ SUMMARY ================================"
printf '%-26s %s\n' "outdir" "$OUT"
printf '%-26s %s\n' "region (logical px)" "$REGION"
printf '%-26s %s\n' "requested fps" "$FPS"
echo "------------------------------------------------------------------------"
printf '%-26s %-22s %s\n' "" "SETTLED (5 s)" "TRANSITION (4 s)"
printf '%-26s %-22s %s\n' "frames captured" \
  "$(field "$S" 'FRAMES' '[0-9]+$')" "$(field "$M" 'FRAMES' '[0-9]+$')"
printf '%-26s %-22s %s\n' "drop rate" \
  "$(field "$S" 'FRAMES' 'drop -?[0-9.]+%')" \
  "$(field "$M" 'FRAMES' 'drop -?[0-9.]+%')"
printf '%-26s %-22s %s\n' "achieved fps" \
  "$(field "$S" 'INTERVAL' '[0-9.]+ fps')" \
  "$(field "$M" 'INTERVAL' '[0-9.]+ fps')"
printf '%-26s %-22s %s\n' "interval median / p95 ms" \
  "$(field "$S" 'INTERVAL' 'median [0-9.]+ ms.*p95 [0-9.]+ ms' | sed -E 's/median ([0-9.]+) ms.*p95 ([0-9.]+) ms/\1 \/ \2/')" \
  "$(field "$M" 'INTERVAL' 'median [0-9.]+ ms.*p95 [0-9.]+ ms' | sed -E 's/median ([0-9.]+) ms.*p95 ([0-9.]+) ms/\1 \/ \2/')"
printf '%-26s %-22s %s\n' "duplicate frames" \
  "$(field "$S" 'DUPLICATES' '^[0-9]+')" "$(field "$M" 'DUPLICATES' '^[0-9]+')"
printf '%-26s %-22s %s\n' "spike frames" \
  "$(field "$S" 'SPIKES' '^[0-9]+')" "$(field "$M" 'SPIKES' '^[0-9]+')"
printf '%-26s %-22s %s\n' "frames inside morph (160ms)" \
  "n/a" "$(field "$M" 'MORPH WINDOW' '[0-9]+ frame')"
printf '%-26s %-22s %s\n' "distinct in morph" \
  "n/a" "$(field "$M" 'distinct' 'distinct [0-9]+' | sed 's/distinct //')"
echo "------------------------------------------------------------------------"
printf '%-26s %s\n' "SUSTAINED (dictation bars, 3 s)" "$OUT/sustained.txt"
printf '%-26s %s\n' "  frames captured" "$(field "$U" 'FRAMES' '[0-9]+$')"
printf '%-26s %s\n' "  achieved fps" "$(field "$U" 'INTERVAL' '[0-9.]+ fps')"
printf '%-26s %s\n' "  duplicate frames" "$(field "$U" 'DUPLICATES' '^[0-9]+')"
printf '%-26s %s\n' "  spike frames" "$(field "$U" 'SPIKES' '^[0-9]+')"
echo "------------------------------------------------------------------------"
echo "READ THIS AS: the SETTLED column is the control. Any duplicate or spike"
echo "there is measurement noise, not a panel fault (a static pill cannot"
echo "redraw). Subtract that noise from the TRANSITION column before blaming"
echo "the animation. Duplicates in TRANSITION that fall INSIDE the 160 ms morph"
echo "are real stalled redraws; duplicates before/after it are just stillness."
echo "Full reports: $S  $M"
} | tee "$OUT/summary.txt"
