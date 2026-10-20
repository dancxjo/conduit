# Quantity projection through an ordinary numerical Fore

The authored Plot applies `math/near(limit = 0.01m)` to at most 64 Quantity
Values. Each input is a canonical Quantity, and each output carries a verified
NearQuantity receipt tied to that exact input and the declared physical limit.

From the repository root:

```sh
printf '0.3m\n1.7m\n' | conduit run plots/quantity-projection/main.conduit
```

The stdin Fore prints one JSON record per accepted result, including the selected
representation and its numeric value when binary32 is selected. Empty input
closes normally. A malformed Quantity, an incompatible physical dimension, or a
65th Value refuses instead of silently starting another Play. Add
`--report report.json` to retain the native Source/Plan/Body/Play and child Sign
report, or `--artifact-directory new-directory` for separate native artifacts.

One installed Std Host and one sealed Body execute the complete input stream.
The pure child numerical kernel is reused only after the preceding output is
accepted. Every invocation has a distinct child Play and retained mandatory
Sign evidence; all finite invocation reservations exist before Play.

This is a Quantity numerical product path. It does not load private neural
weights or claim FARGAN adoption.
