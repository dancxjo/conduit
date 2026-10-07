#!/bin/sh
# Development only; never fetches or redistributes pretrained tensors.
set -eu
if [ "$#" -ne 2 ]; then
    printf 'Usage: %s PINNED_OPUS_SOURCE_DIRECTORY DEVELOPMENT_FIXTURE_DIRECTORY\n' "$0" >&2
    exit 2
fi
fargan_source_dir=$(realpath "$1")
fargan_fixture_dir=$(realpath "$2")
fargan_probe_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
printf '%s  %s\n' '68ba6e8731007f7184be862d7d1afe4cef096a7c9bcbfa115962340f725e004c' "$fargan_source_dir/dnn/fargan_data.c" | sha256sum --check
# Full float oracle: DISABLE_DEBUG_FLOAT must remain undefined. The compact
# profile is a separate build/evidence class, even with scalar kernels.
cc -O2 -std=c99 -ffp-contract=off -DOPUS_BUILD -DSUPPRESS_PERF_WARNINGS \
    -U__SSE2__ -U__SSE__ -fno-tree-vectorize \
    -I"$fargan_source_dir/dnn" -I"$fargan_source_dir/celt" \
    -I"$fargan_source_dir/include" -I"$fargan_source_dir" \
    -ffunction-sections -fdata-sections \
    "$fargan_probe_dir/conditioning_oracle.c" \
    "$fargan_source_dir/dnn/nnet.c" "$fargan_source_dir/dnn/nnet_default.c" \
    "$fargan_source_dir/dnn/parse_lpcnet_weights.c" "$fargan_source_dir/dnn/fargan_data.c" \
    -Wl,--gc-sections -lm -o "$fargan_fixture_dir/conditioning-float-oracle"
"$fargan_fixture_dir/conditioning-float-oracle" "$fargan_fixture_dir/conditions.f32le" "$fargan_fixture_dir/conditioning.trace"
sha256sum "$fargan_fixture_dir/conditioning.trace"
