#!/usr/bin/env bash
# Render every shader to its own video for quick eyeballing.
# Usage: ./render_all.sh [input.mp3]   (default: sample-10s.mp3)
set -euo pipefail
cd "$(dirname "$0")"

input="${1:-sample-10s.mp3}"
outdir="renders"
mkdir -p "$outdir"

cargo build --release

for shader in shaders/without_audio/*.glsl; do
    name=$(basename "$shader" .glsl)
    out="$outdir/${name}.mp4"
    if [ -s "$out" ]; then
        echo "skip $out (exists)"
        continue
    fi
    echo "==> $out"
    # These shaders animate on time alone; --duration-only skips audio analysis.
    ./target/release/SpectraForge --input "$input" --shader "$shader" --output "$out" --duration-only
done

echo "done -> $outdir/"
