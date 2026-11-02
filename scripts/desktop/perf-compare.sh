#!/usr/bin/env bash
# Phase 2 gate measurements: CPU and memory of Cortex vs Rec on Rec's synthetic test
# pattern (no Screen Recording permission needed), plus frame counts from ffprobe.
#
#   scripts/desktop/perf-compare.sh cortex 1080p60-h264 60 OUTDIR   record N s in Cortex.app (main window + live preview)
#   scripts/desktop/perf-compare.sh rec    4k60-hevc   60 OUTDIR    same with Rec.app's `rec-rs record`
#   scripts/desktop/perf-compare.sh idle   preview-on  30 OUTDIR    Cortex idle, main window open (preview-on | preview-off)
#
# Env: CORTEX_APP (default: target/release/bundle/macos/Cortex.app), REC_BIN
# (default: /Applications/Rec.app/Contents/MacOS/rec-rs).
# CPU is measured from each process's cumulative CPU time (ps `time`), sampled every
# second: average = total CPU seconds / wall seconds; p95 over the 1 s samples. 100% = one core.
# Cortex includes the WebKit processes that start with it (WebContent, GPU, Networking).
set -euo pipefail

mode=${1:?cortex|rec|idle}
config=${2:?1080p60-h264|4k60-hevc|preview-on|preview-off}
secs=${3:?seconds}
out=${4:?output dir}
root=$(cd "$(dirname "$0")/../.." && pwd)
app=${CORTEX_APP:-$root/target/release/bundle/macos/Cortex.app}
rec_bin=${REC_BIN:-/Applications/Rec.app/Contents/MacOS/rec-rs}
mkdir -p "$out"
tag="$mode-$config-${secs}s"
log="$out/$tag.log"
samples="$out/$tag.samples.tsv"
: >"$log"

case $config in
  1080p60-h264) res=1080p codec=h264 ;;
  4k60-hevc) res=4k codec=hevc ;;
  preview-on | preview-off) res=1080p codec=h264 ;;
  *) echo "unknown config $config" >&2; exit 2 ;;
esac

webkit_pids() { pgrep -f 'com.apple.WebKit' | sort -n || true; }

# Samples CPU time of the pids listed (one per line) in $1 until $2 exits; prints a summary.
sample() {
  python3 - "$1" "$2" "$samples" <<'PY'
import subprocess, sys, time, os
pidfile, watch, out = sys.argv[1], int(sys.argv[2]), sys.argv[3]
def cpu_secs(t):
    # [[dd-]hh:]mm:ss.ss
    d = 0
    if '-' in t: dd, t = t.split('-'); d = int(dd) * 86400
    parts = [float(p) for p in t.split(':')]
    s = 0.0
    for p in parts: s = s * 60 + p
    return d + s
def read():
    pids = [p.strip() for p in open(pidfile) if p.strip()]
    if not pids: return {}, 0
    r = subprocess.run(['ps', '-o', 'pid=,time=,rss=', '-p', ','.join(pids)], capture_output=True, text=True)
    cpu, rss = {}, 0
    for line in r.stdout.split('\n'):
        f = line.split()
        if len(f) == 3:
            cpu[f[0]] = cpu_secs(f[1]); rss += int(f[2])
    return cpu, rss
start = time.time(); prev, _ = read(); t_prev = start
rows, total, peak = [], 0.0, 0
with open(out, 'w') as fh:
    fh.write('t\tcpu_pct\trss_mb\n')
    while True:
        time.sleep(1)
        alive = os.path.exists(f'/proc/{watch}') or subprocess.run(['kill', '-0', str(watch)], capture_output=True).returncode == 0
        cur, rss = read(); now = time.time()
        d = sum(max(0.0, v - prev.get(k, 0.0)) for k, v in cur.items() if k in prev)
        # A pid seen for the first time: count its CPU since the last sample as unknown (skip).
        pct = 100.0 * d / (now - t_prev)
        if cur:
            rows.append(pct); total += d; peak = max(peak, rss)
            fh.write(f'{now-start:.1f}\t{pct:.1f}\t{rss/1024:.1f}\n')
        for k, v in cur.items(): prev[k] = v
        t_prev = now
        if not alive: break
wall = len(rows)
s = sorted(rows)
p95 = s[min(len(s) - 1, int(0.95 * len(s)))] if s else 0
avg = 100.0 * total / wall if wall else 0
print(f'cpu_avg_pct={avg:.1f} cpu_p95_pct={p95:.1f} rss_peak_mb={peak/1024:.0f} samples={wall}')
PY
}

probe() {
  local f=$1
  ffprobe -v error -count_frames -select_streams v:0 \
    -show_entries stream=codec_name,width,height,r_frame_rate,nb_read_frames:format=duration \
    -of default=noprint_wrappers=1 "$f" | tr '\n' ' '
  echo
}

pidfile="$out/$tag.pids"
case $mode in
  cortex | idle)
    bin=$(ls "$app/Contents/MacOS/" | head -1)
    data="$out/$tag.data"
    rm -rf "$data"; mkdir -p "$data/out"
    cat >"$data/settings.json" <<JSON
{"version":1,"outputDir":"$data/out","container":"mov","codec":"$codec","resolution":"$res","fps":60,
 "scene":"screen","micEnabled":false,"systemAudio":true,"countdown":0,"replayEnabled":false}
JSON
    before=$(webkit_pids)
    env_args=(CORTEX_DEBUG_DATA_DIR="$data" CORTEX_DEBUG_TEST_PATTERN=1 CORTEX_DEBUG_PREVIEW_STATS=1)
    [[ $mode == cortex ]] && env_args+=(CORTEX_DEBUG_RECORD_SECS="$secs")
    [[ $config == preview-off ]] && env_args+=(CORTEX_DEBUG_HIDE_MAIN=1)
    env "${env_args[@]}" "$app/Contents/MacOS/$bin" >>"$log" 2>&1 &
    pid=$!
    sleep 2
    echo "$pid" >"$pidfile"
    comm -13 <(echo "$before") <(webkit_pids) >>"$pidfile"
    echo "pids: $(tr '\n' ' ' <"$pidfile")" >>"$log"
    if [[ $mode == idle ]]; then
      # Settle, then measure for $secs and quit.
      sleep 3
      ( sleep "$secs"; kill "$pid" 2>/dev/null ) &
    fi
    summary=$(sample "$pidfile" "$pid")
    wait "$pid" 2>/dev/null || true
    file=$(sed -n 's/^cortex: recorded //p' "$log" | tail -1)
    ;;
  rec)
    file="$out/$tag.mov"
    rm -f "$file"
    "$rec_bin" record --test-pattern --no-mic -r "$res" -f 60 --codec "$codec" -t "$secs" -o "$file" >>"$log" 2>&1 &
    pid=$!
    echo "$pid" >"$pidfile"
    summary=$(sample "$pidfile" "$pid")
    wait "$pid" || echo "rec-rs exited $?" >>"$log"
    ;;
  *) echo "unknown mode $mode" >&2; exit 2 ;;
esac

{
  echo "== $tag"
  echo "$summary"
  if [[ -n ${file:-} && -f $file ]]; then
    echo "file=$file"
    echo "probe: $(probe "$file")"
  fi
  grep -E 'preview [0-9.]+ fps' "$log" | tail -3 || true
  grep -iE 'drop|dup|error|fail' "$log" | tail -5 || true
} | tee "$out/$tag.summary"
