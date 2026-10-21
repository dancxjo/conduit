# Owner custody storage repair isolated on current dev

The original repaired owner implementation is retained unchanged in two source files, without the unrelated browser/Look/tooling preparation or large capture archive from the earlier branch. Tested source fedb4884127d88ff8ce6493c75af11062253bd8d is based on e54605b7696ab993e7875a04b0d821304d4d4afb.

Eight actual owner binary custody tests passed, zero failed or ignored. The regression retains a valid40,000-byte array receipt whose pretty representation exceeds the192KiB index-reader bound, then verifies compact storage, current receipt, recovery and history. Existing finite-capacity/corruption/replacement tests remain included. The first library-only invocation selected zero tests; it is not used as proof.

This is a minimal admission candidate for the responsible storage bug. Original source-bound browser evidence remains on fix/6209-owner-refusal-evidence at4ff62494ef26bfc2ad94f88315fac693ec209990. It is not relabeled as a fresh capture of this isolated checkout. Full6209 still requires current-source supported browser/delivery acceptance.5630/5645 remain open. No new PR or CI/native/browser cycle was started.
