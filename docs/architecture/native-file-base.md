# Native Patchbay protected file base

Native Patchbay's first optional desktop family reuses the standard `file/copy` checked face and bounded copy implementation. The native file dialog is only a base for user choices. It does not define a new operation, planner, scheduler, or task result.

The base is composed only when a display connection and a usable dialog executable are present. The conforming omitted-base composition uses the same minimal host with no file capability or protected-file resource advertisement. A click while the base is absent or cancellation of the dialog creates no grant.

Selected paths cross directly into `ProtectedFileRegistry`, whose private table maps them to the two fixed boot-scoped opaque handles. The semantic Form, checked and expanded identities, Plan, fragment, Play, receipt, kernel protocol, and Patchbay presentation contain the handles and exact access, byte, and commit bounds, never the raw locators. Re-selecting a role revokes its earlier handle and invalidates any prepared Plan.

F7 chooses the read-existing source. F8 chooses a create-only destination; Shift-F8 chooses replace-existing. F9 invokes the shared ordinary protected-resource planning recipe. F10 runs the resulting fragment through `StdHost::run_copy_fragment` and the production kernel. F11 sets the existing bounded copy Stop token. The resulting receipt preserves the exact request, active Play, Plan, source and destination handles, structured result, and kernel clue count.

`--native-copy-demo` is a finite platform-acceptance aid: it opens the same two native dialogs at startup and then invokes the same Plan and Run methods. It accepts no locator arguments and does not bypass either protected grant.

The base does not manufacture success. Base failure, dialog cancellation, missing choices, planning rejection, destination conflict, denial, stale handle, oversize, partial copy, cancellation, and cleanup failure remain distinct at their existing boundaries. Raw paths may be shown by the platform dialog itself, but are not promoted into Conduit semantic or execution identity.
