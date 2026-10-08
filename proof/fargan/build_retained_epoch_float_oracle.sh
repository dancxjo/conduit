#!/bin/sh
# Development oracle only. Caller supplies privately acquired pinned source/model.
set -eu
if [ "$#" -ne 3 ]; then
  echo 'usage: build_retained_epoch_float_oracle.sh OPUS_SOURCE MODEL_DATA_DIRECTORY OUTPUT' >&2
  exit 2
fi
opus=$(realpath "$1")
model=$(realpath "$2")
output=$(realpath -m "$3")
script=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
expected=503d81b138d76621aae4b12786e90de48aa8db3a
if [ -e "$opus/.git" ] && [ "$(git -C "$opus" rev-parse HEAD)" != "$expected" ]; then
  echo 'refusing unpinned opus source' >&2
  exit 3
fi
# Pins the selected upstream numerical files even for a source archive.
printf '%s  %s\n' 'fca4e51f0407f9791900a972119a69f724485f034e48a2e77a608d24d950cc2c' "$opus/dnn/fargan.c" | sha256sum -c -
printf '%s  %s\n' '1b376a8499167e143f8a4ec8d0c622d259b4ae99d8c7495a80d6318912de81e7' "$opus/dnn/nnet.c" | sha256sum -c -
printf '%s  %s\n' '1402eb611b7fcc1fe7fa18639f0395e0af47a783ef87450318411483516e9011' "$opus/dnn/nnet_default.c" | sha256sum -c -
printf '%s  %s\n' '78ab42e0fc04a72f89bc1a1af9363c630263608cb1065751c2c4d89c5f93e89a' "$opus/dnn/parse_lpcnet_weights.c" | sha256sum -c -
printf '%s  %s\n' 'fbdf49c54d60cfaa81607caef2f2d83bebde51de2a53aee2607b114350425a8e' "$opus/dnn/vec.h" | sha256sum -c -
printf '%s  %s\n' 'e1761b61669f70ed6ae918772b8fae3ed9fe5c65f67d2ce3d68749ac4bd9a4ad' "$opus/dnn/fargan.h" | sha256sum -c -
printf '%s  %s\n' 'be76402755570a7d9251f1cb9e9960792378dbb52bb5273f5478ceeb83571d10' "$opus/dnn/nnet.h" | sha256sum -c -
printf '%s  %s\n' '3b227cd54ae376c21050a1116261665c9c8d82b9ec46b1c9aed9af6a6cd444d0' "$opus/dnn/nnet_arch.h" | sha256sum -c -
printf '%s  %s\n' '68ba6e8731007f7184be862d7d1afe4cef096a7c9bcbfa115962340f725e004c' "$model/fargan_data.c" | sha256sum -c -
if [ -e "$opus/dnn/fargan_data.c" ]; then printf '%s  %s\n' '68ba6e8731007f7184be862d7d1afe4cef096a7c9bcbfa115962340f725e004c' "$opus/dnn/fargan_data.c" | sha256sum -c -; fi
printf '%s  %s\n' '402e691f4975d6d156903f0fe12da3385e20ec6dd6155c3387e9dad536f654cd' "$model/fargan_data.h" | sha256sum -c -
if [ -e "$opus/dnn/fargan_data.h" ]; then printf '%s  %s\n' '402e691f4975d6d156903f0fe12da3385e20ec6dd6155c3387e9dad536f654cd' "$opus/dnn/fargan_data.h" | sha256sum -c -; fi
# No source/model is downloaded or copied. Omitting DISABLE_DEBUG_FLOAT is
# required: scalar alone does not select the full-float precision profile.
cc -O2 -std=c99 -ffp-contract=off -DOPUS_BUILD -DSUPPRESS_PERF_WARNINGS \
  -U__SSE2__ -U__SSE__ -fno-tree-vectorize \
  -I"$model" -I"$opus/dnn" -I"$opus/celt" -I"$opus/include" -I"$opus" \
  -ffunction-sections -fdata-sections \
  "$script/retained_epoch_float_oracle.c" "$opus/dnn/nnet.c" \
  "$opus/dnn/nnet_default.c" "$opus/dnn/parse_lpcnet_weights.c" \
  "$model/fargan_data.c" -Wl,--gc-sections -lm -o "$output"
