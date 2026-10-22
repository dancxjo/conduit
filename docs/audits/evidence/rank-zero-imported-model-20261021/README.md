# #6210 local rank-zero imported inference proof

The complete supported `cargo +stable xtask check model-inference` passed at
clean Source `9cacd3ca8d8ae486112ba81d1f35b692c25505f1`: **68 tests, 24 targets, zero failures or ignored tests**.
The raw command output is retained as deterministic gzip; `manifest.json` records
raw and compressed hashes, commands, source boundaries and pinned graph hashes.

The checked-in I64 and F32 Identity graphs have explicit empty shapes. Authored
Source selects the ordinary inference Gear, exact imported checkpoint/name map
and actual prepared Back identity. Plan inspection keeps zero signature axes and
the `scalar → x`, `y → scalar` correspondence. Play returns the exact one-element
bytes with rank zero. A rank-one length-one tensor remains a distinct value.

The scalar tests additionally cover raw tensor encoding, canonical Native and
borrowed bundle admission, exact resource extents, and dtype-width storage.
Twelve invalid input cases check zero provider calls and zero emitted outputs;
four deliberately invalid provider replies check zero emitted outputs. Unknown
ONNX shapes and zero dimensions still refuse, distinctly from declared scalars.

The initial focused Data test compile failure is preserved. It selected the
consuming Native trait method rather than the borrowed raw tensor codec; the
explicit method fix is included in the tested Source. The complete clean-source
suite supersedes the intermediate focused runs for acceptance.

This is local hosted fixture proof. CI, exact-head Integration, merge and issue
closure remain delivery gates. It establishes neither actual ORT execution nor
completion of parent #5219; optional inputs, numeric finiteness policies,
external file admission and rendered Patchbay proof retain their separate scope.
