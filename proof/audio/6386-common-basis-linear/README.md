# Common-basis linear Audio conformance (#6386)

The explicit CommonBasisU32 profile admits finite exact linear frequency, cycle, amplitude and power controls on a retained common rational basis. The default U8 linear and existing U32 step paths remain unchanged. All32 preexisting compiled Source program values are byte-identical; three new Source programs are recorded separately.

Generated Native admission bounds each numerator/denominator by U32MAX, requires matching bases/domains, and readmits coverage before subtraction. For interior interpolation the nonnegative weights sum to span: weighted numerator <=max(endpoint numerators)*span <=(2^32-1)^2 <U64MAX. Denominator <=(2^32-1)^2 and positive. Endpoint branches return the authored fraction exactly. One shared linear time denominator is admitted during preparation.

At most16 segments produce94 preparation records and at most18 evaluation records. Canonical inputs, profile, selected segment, program and output remain in each successful receipt. This is an allocating preparation/conformance API, not a no-growth Play Back.

The installed-source test takes the actual admitted Covered input from production evaluation, installs the same checked Source through the ordinary planner and StdHost kernel, and compares byte-exact outputs. Its shared harness checks Plan/ActivePlay identity, fixed value allocation capacity and HostCall request/completion. No forged-input confinement, PCM, physical playback, auditory quality or parent #5680 completion follows.

Validation results and exact source/log hashes are recorded in results.json. Exact-head Candidate and postmerge Integration remain separate acceptance gates.
