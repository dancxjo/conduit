# Owner Face browser text preference

Source `88f5cb4b81ca0b12d1ef1832e82b0a63168efa9b` fixes root text scaling across the owner component shadow boundary. Font sizes use rem while the admitted Look scale remains an independent multiplier. Default 16px-root geometry is preserved.

The new browser check fails on the previous CSS: after a 200% root preference the button remains 13px instead of 26px (`before.log`). On the final source, the same retained control grows to 26px without a new Show; adding the admitted 200% scale gives 52px. At 390px width the document does not overflow horizontally, the keyboard-focused button remains reachable, and forced colors/reduced motion apply.

All 141 owner renderer, reachability/module and Look-control Chromium checks pass, one worker, no retries (`checks.log`). `owner-root-text-forced.png` shows the combined 400% case, deliberately tall and wrapped rather than clipped. This is a renderer fixture, not an owner action receipt, actual native enactment or complete cross-component Look acceptance. The separate Quantity runtime failure #6387 remains open. No CI was dispatched.
