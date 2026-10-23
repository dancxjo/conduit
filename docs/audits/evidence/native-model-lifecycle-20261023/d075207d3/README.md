# Native CPU model lifecycle proof

Measured source: `d075207d3f49e5ac4ef9cbc5f270de6f892cfb44`.

The supported `cargo +stable xtask check model-native-inference --locked` passed on its first attempt: 26 tests, no failures or ignored tests. The receipt retains exact executable, worker and ONNX Runtime hashes. Actual executable binaries remain in the external proof directory; this packet retains the raw log and independent review.

The suite covers bounded preparation pipe deadlines and nonblocking transport, pending request custody, exact response identity, stale/cancel/drop/provider-loss refusals, ordinary Source→Plan→Play, and actual native CPU execution. Actual native lifecycle controls reuse the same prepared child across two distinct Plays, cancel a confirmed stopped worker with its exact request queued, and reject an actually killed worker without hidden retry. Cancellation revokes publication before worker cleanup. These controls establish pending cancellation and child reaping, not instruction-level CPU interruption or hard real-time execution bounds.

The original native inference controls retain nineteen exact IEEE-754 results and zero parent Play allocation calls/requested bytes. Deferred fixture tests remain explicitly separate from actual native worker tests. No downloads, extra production scheduler or custom operation are introduced. Default/NoStd and native optional boundaries remain distinct.

Development proof does not establish accepted Integration or stable publication. The implementation issue remains open until the scoped change is merged with relevant accepted checks.
