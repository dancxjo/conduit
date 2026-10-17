# Keep std kernel preparation off the caller's wide Result frames

The Candidate for #5231's production-kernel measurements failed the existing Patchbay read-only routes test with a native stack overflow. The failure is in `exact_read_only_routes_are_bounded_no_store_and_typed` in `conduit-browser-patchbay-workbench --test server`, on the unchanged default test-thread stack. Source 9fff9e82256b0a4c2e0d9376bda99d2fb4ccb93d reproduces the same abort locally with the CI debug settings and RUST_MIN_STACK absent.

`KernelTables::install` already returns a heap-owned scheduler, but previously obtained a full inline scheduler from `new_with_active_counts_and_host_calls`, then moved it into a Box. It now calls the kernel's existing allocating-preparation entrance, `new_boxed_with_active_counts_and_host_calls`, and returns that Box. A pointer crosses the outer Result boundary instead of the wide scheduler value. The constructor retains the same fixed tables, active counts, Host bindings, drivers, storage and Signs. Fusion and terminal-contract binding still happen before return. Allocating preparation remains separate from fixed-storage Play.

The same read-only routes test passes after this two-line production change, using the same compiler/profile and unchanged default test stack. No harness stack override or test assertion changes are involved. This proves the observed hosted preparation failure is repaired; it is not a portable minimum-stack or physical-target memory guarantee. The fixed constructor remains available for its existing fixed-storage callers.

At repair source `634257f89d1ab5fa70f4f1855e2c929a49bcc9d8`, the full browser-workbench all-target suite passes 62 tests, including all 13 server tests and four snapshot tests. Strict std Host and browser-workbench all-target Clippy denies warnings and passes. Workspace formatting and diff whitespace checks pass. All 11 archived artifact hashes verify. The existing default-stack server test is the regression; its production assertions remain unchanged.

The failure was first retained by [Candidate job 117503350781](https://github.com/dancxjo/conduit/actions/runs/39069133638/job/117503350781). The local before/after logs, compiler and source-file/executable identities, CI failure excerpt and patch are archived in `evidence/std-preparation-stack-20261017-raw.tar.gz`; `evidence/std-preparation-stack-20261017.json` indexes those observations. Baseline runtime code is current dev, and the Candidate's native files match it: that preview differs only in the four large-sparse measurement/audit files. The unrelated platform integration repair retains its own owner and proof.

This is a prerequisite repair for #5231's failing gate. The completion-count optimization remains on its separate held branch; full #5231 acceptance, including its required FARGAN graph measurements, is not asserted by this fix.


## Preserve early canvas control refusal

The same Candidate's [foundation job](https://github.com/dancxjo/conduit/actions/runs/39069133638/job/117503350633) failed the resident-canvas negative test at its unwrap of an unknown-subject event encoding. Production Face admission already correctly refuses that value; the test was attempting to deliver a value which the trusted encoder would not create. The failure reproduces at `aff7f393ec6dc7e3a5b20e0dff0f5c7bd6e9bd95` under the foundation opt-level-one test profile.

The regression now asserts encoder refusal, then replaces only the value bytes of a valid same-length wire event with an explicitly unlisted subject. The consumer must refuse that forged frame with `InvalidControlValue`, preserving both selected inspection and the whole current view. Valid selection and stale-revision checks remain. The production admission guard's bytes are unchanged.

At `976468ad5a933c72a0f6a7799c6bda1d9f2bf914`, all 44 Patchbay application all-target tests and strict all-target package Clippy pass. Source identities, logs, the CI failure excerpt and six verified archived artifact hashes are retained in `evidence/canvas-control-refusal-20261017.json` and the matching raw archive. This test correction and the boxed-preparation change address the two observed Patchbay Candidate failures; neither relaxes production validation.


After merging admitted dev `d1eba3e5eee970913a2627158c0c99ed8b9cd7f6` without rewriting either evidence ancestry, source `f667e4d02961587e463ad6937dcd96b23c80d4f9` again passes the unchanged default-stack routes test and all 44 Patchbay application tests. The replay retains its own source/log identities in `evidence/patchbay-ci-repair-current-dev-20261017.json` and matching raw archive (three artifact hashes verified). Prior full-package and Clippy receipts retain their original sources; these local rechecks do not substitute for the fresh combined Candidate.
