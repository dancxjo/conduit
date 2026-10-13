Native participants cannot use the existing invitation joining protocol while its document types live privately in the CLI. Move those same invitation, request, and receipt types and their existing validation into the authenticated-admission module of conduit_body, then make the CLI use the shared implementation.

The serialized shapes and unknown-field policies stay compatible. Document validation grants no membership: the existing admission manager still verifies and consumes invitation authority, and the caller must authenticate the Body owner. No new protocol, transport, or native joining claim is introduced.

Prerequisite for #4807; does not close it. Validation passed: six document compatibility/refusal tests, both existing CLI joining integration tests, strict body Clippy, formatting, and no_std authenticated-admission compilation for x86_64-unknown-none. Workspace-wide admission gates remain for CI.
