The shared kernel can return from payload fanout after mutating one queue but before completing publication. The public safe `SignSink::record` is fallible after `ensure_capacity`; current rollback releases only one staged-output reference, leaving references reserved for recipients that never received the value.

Source audit against d863617e78 (not a fault-injection reproduction): materialize V (ref1), retain for three recipients (ref3), push A, fail its ValueRouted Sign, rollback one reference (ref2), consume A (ref1). The remaining reference has no owner. Public step permits retry; repeated failure/drain can accumulate unreachable values. Built-in HostedSignLog preflights sequence capacity; this is not evidence of failure with that sink in normal operation.

This blocks treating the successful PreserveOrder FIFO suffix proof as a general reduced physical-stock guarantee. Successful atomic multicast needs at most max sibling queue capacity, but partial commits can break that premise. Keep current admitted stock unchanged until the failure contract is qualified.

Existing owners: #6341 preparation/workspace admission, #6340 reservation custody and #6409 failure/lifetime retirement. This is a specific shared-kernel correctness obligation, not a new MCU-memory epic.

Acceptance:
- Inject fallible Sign/storage errors at each fanout route boundary through the production API.
- Establish transactional rollback, guaranteed post-preflight commit, or an explicit faulted-until-retired state with complete cleanup.
- Verify refs, queues, readiness and driver ownership after refusal, retry, cancellation and rearm.
- Only then gate any reduced fanout stock on the proved contract; preserve logical capacities, IDs and generations.

Detailed source audit will be preserved with the greenhouse checkpoint PR #6902 (`fanout-proof-review-20261115.md`). No proposed fanout reduction has been enabled.
