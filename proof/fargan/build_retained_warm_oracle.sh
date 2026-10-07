#!/bin/sh
set -eu
if [ "$#" -ne 3 ]; then
    echo 'usage: build_retained_warm_oracle.sh PINNED_DEVELOPMENT_ROOT ARTIFACT_DIRECTORY SCRATCH_DIRECTORY' >&2
    exit 2
fi
model_root=$1
artifact_root=$2
scratch_root=$3
script_root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
mkdir -p "$scratch_root"
# External local pinned503d81b development sources/arrays; no download or weights.
# DISABLE_DEBUG_FLOAT deliberately omitted: full float32 matrices, scalar only.
gcc -O2 -std=c99 -ffp-contract=off -DOPUS_BUILD -DSUPPRESS_PERF_WARNINGS -U__SSE2__ -U__SSE__ -fno-tree-vectorize -I"$model_root/oracle/dnn" -I"$model_root/oracle/celt" -I"$model_root/oracle/include" -I"$model_root/oracle" -ffunction-sections -fdata-sections "$script_root/retained_warm_oracle.c" "$model_root/oracle/dnn/nnet.c" "$model_root/oracle/dnn/nnet_default.c" "$model_root/oracle/dnn/parse_lpcnet_weights.c" "$model_root/oracle/dnn/fargan_data.c" -Wl,--gc-sections -lm -o "$scratch_root/warm_probe"
python3 "$script_root/retained_warm_inspection.py" "$artifact_root" "$scratch_root"
"$scratch_root/warm_probe" "$scratch_root/actual-warm-five.f32le" "$scratch_root/pinned-warm-oracle.bin"
python3 "$script_root/retained_warm_comparison.py" "$scratch_root" "$model_root"
