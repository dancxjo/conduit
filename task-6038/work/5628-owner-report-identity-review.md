# Current owner report identity review

Source: reconciled253dcb985; read-only review before actual owner/guest proof.

The proposed browser helper initially treated owner-mask-wardrobe report `owner_plan_id` as the workload Body Plan and required it unchanged across Look swaps. This is wrong. `products/conduit/src/body_owner/face_wardrobe_report.rs` serializes `wardrobe.plan().plan_id`, the outer `OwnerFacePlan`. `face_wardrobe.rs::admit_or_replace` explicitly replaces that Plan after unknown/replaced child witnesses or stale Face/Body. `face_wardrobe_runtime.rs::admit_native_face_route` uses this path on native route replacement.

Wear/Doff/Prefer against an unchanged admitted route set must retain the same outer Plan, as the existing three-Host proof requires. A Look swap may reseal both child route and outer Face Plan. Its fresh independent report must bind Body, Face/revision, exact acknowledged Show and selected route ID to the new native route Plan; policy revision can reset after Plan replacement. Do not require a different revision alone to establish freshness. One deliberately requested Inspect, completed without refusal, obtains the current report; passive predicates never repeat that action.

Complete Face/component/recipe correspondence and actual owner snapshots remain required. Do not relabel outer Face Plan identity as workload Plan identity. The helper owner is correcting this before execution; no browser/owner/guest run has been claimed.
