# Blue selected kernel backing versus Fore costs

Read directly from the old reviewed 97fc and new 777e ELFs; no rebuild or device operation.

| Exported selected-kernel metric | Old | New | Saved |
|---|---:|---:|---:|
| Sampling |16,186|6,466|9,720|
| Output |3,822|2,142|1,680|
| Native Mask |6,200|6,192|8|
| Total |26,208|14,800|11,408|

The metric includes each kernel and its existing separately defined Sign backing. The intentional sampling/output reduction is 11,400 bytes; the additional 8 bytes are a measured cross-version underlying Mask layout difference, not a Mask specialization. These heap-backed kernel savings are separate from the 11,136-byte static async-task reduction.

The new Fore owns three exact-capacity heap payload buffers: 1,024-byte carrier frame, 8-byte ingress and 837-byte egress: **1,869 requested bytes**, excluding allocator overhead. Production retained-backing paths report carrier 152+1,024, bridge 224+8+837, and sessions 360, totaling 2,605. The existing caller adds Hot 1,428 and Vec headers 36: known Fore-related retained charge **4,069 bytes**, including 2,200 inline owner bytes. Inline owner charges are not additional heap allocations. Cold Lines/session metadata is retired before kernel funding; its preparation overlap remains unmeasured.

Across only these known categories, kernel metric reduction minus new payload buffers is 9,539 bytes. This is not a measured whole-factory heap difference or peak.

A concrete remaining traffic gate was found: the sampling constructor still uses `HostedSignLog::new`, which provides **zero remote Sign entries**. Actual Line ingress checks remote evidence capacity before allocation; the bridge converts that exhaustion into pressure. Linking the candidate therefore does not establish working history traffic. Finite remote Sign funding/retirement must be resolved without weakening evidence.
