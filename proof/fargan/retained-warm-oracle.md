The retained warm oracle compares actual Source startup outputs with pinned
Opus revision `503d81b138d76621aae4b12786e90de48aa8db3a`. It uses the exact
first feature row retained in `session-basis.bin`, repeats that row for the
five conditioning calls, and passes the declared zero 320-sample continuation
seed to upstream `fargan_cont`. It compares 128 conditioning-history values,
837 active signal-state values, and the retained period.

```sh
sh proof/fargan/build_retained_warm_oracle.sh \
  /path/to/local/pinned-development \
  /path/to/retained-source-artifact \
  /path/to/scratch
```

The development directory supplies the previously acquired `oracle/dnn`,
`oracle/celt`, `oracle/include`, and `resources` files. The script verifies
pinned source digests, generated model-source digest, and full float blob
identity. No weights are included or downloaded. The artifact directory
supplies `manifest.json`, `session-basis.bin`, and `raw-epochs.json`.

The canonical decoder inspects previously retained artifacts; it does not
admit Native laws or execute Source. The C oracle is development-only. Source
uses libm activations while upstream uses its scalar approximations, so the
report measures error without claiming bit parity. It does not compare warm
PCM or subsequent utterance PCM: later actual feature rows are not retained
by the original artifact. An acoustic-quality conclusion needs that complete
input trace and a separate listening result.
