# Step-budget cancellation diagnostics

The exact1f5 x86 product kernel and corrected e245 checked CDC package transmit one12-byte buffer and then wait for the first64-byte IN. The external device retains it. Both runs reach the admitted Root step budget and report protocol-step-budget-exhausted, without claiming normal Play completion.

The held-IN transcript observes cancellation and its zero-byte canceled response. The late-response fixture sends actual bytes only after observing the matching USBredir cancel packet. QEMU may suppress that response before a guest xHCI event; this is not proof of a late admitted Native provider completion. Original receipts remain diagnostic-only.

The inspected execute_workload implementation calls play.cancel successfully before reporting the preserved step-budget refusal; failed cancellation would instead report protocol-retirement-refused. That control-flow observation is weaker than a bound operation retirement receipt. Tests with actual retained native reservations and original Kernel tokens, and explicit retained retirement evidence, remain required. No acceptance or issue closure claim is made here.
