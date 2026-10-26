#!/usr/bin/env bash
# Average CPU use of a running Handset, to compare decoders and settings. Run: scripts/measure-cpu.sh [seconds]
# Start Handset first and scroll or play something on the phone so the video keeps changing.
set -euo pipefail

seconds="${1:-20}"
pid="$(pgrep -x handset | head -1 || true)"
[ -n "$pid" ] || { echo "handset is not running" >&2; exit 1; }

samples=()
for _ in $(seq "$seconds"); do
  samples+=("$(ps -o %cpu= -p "$pid" | tr -d ' ')")
  sleep 1
done
printf '%s\n' "${samples[@]}" | awk '{s += $1; if ($1 > m) m = $1} END {printf "handset pid '"$pid"': average %.1f%% CPU, peak %.1f%% over %d s\n", s / NR, m, NR}'
