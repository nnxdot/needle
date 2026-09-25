#!/usr/bin/env bash
# Needle's benchmark (NEEDLE_BENCH, see PERFORMANCE.md) on Linux, with the installed `needle`:
#   scripts/linux-bench.sh <copy of a data folder> [song to play]
# Needle walks its pages and closes itself; this prints the frame log, the processor time and
# memory, and any warnings, errors, or crashes from Needle's log and output.
set -u
data=${1:?the copy of a data folder to use}
shift
log=${TMPDIR:-/tmp}/needle-frames.log
out=${TMPDIR:-/tmp}/needle-bench.out
rm -f "$log" "$out"
# Only this run's lines of Needle's own log.
before=$(wc -l < "$data/logs/needle.log" 2>/dev/null || echo 0)
start=$(date +%s)
NEEDLE_FRAME_LOG="$log" NEEDLE_BENCH=1 /usr/bin/time -f "cpu %U s user + %S s system, peak memory %M KB" \
    timeout 420 needle --data-dir "$data" "$@" > "$out" 2>&1
echo "exit $? after $(($(date +%s) - start)) s"
cat "$log"
echo "--- Needle's log (this run): warnings, errors, crashes"
tail -n +"$((before + 1))" "$data/logs/needle.log" 2>/dev/null | grep -iE "warn|error|crash|panic" || echo "(none)"
echo "--- output"
tail -n 25 "$out"
ls "$data/crashes" 2>/dev/null | head
