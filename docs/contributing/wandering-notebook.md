# Try the Wandering Notebook

The development notebook keeps one revisioned UTF-8 document in a continuing
Body. The Linux owner executes its checked Source; presentation Masks submit
typed requests against their acknowledged Show. File publication uses an
explicitly selected directory. This first bounded profile accepts at most
256 UTF-8 bytes, including line breaks.

Use an installed Linux `conduit` executable and a private owner state directory
as described in [the owner guide](shared-body-owner.md). Birth its Body with
`conduit body own plots/notebook/main.conduit --state-dir STATE --name Thoughts`,
then close that foreground encounter and run
`conduit host service run --state-dir STATE` in a separate terminal.
The executable must be the installed image selected by that state directory.

Create, write, and commit the document through the public owner commands:

```sh
conduit body notebook --state-dir STATE new --name Thoughts
conduit body notebook --state-dir STATE replace --revision 0 --text 'I was here.'
conduit body notebook --state-dir STATE save --storage-dir STORAGE
conduit body notebook --state-dir STATE show
```

`STORAGE` must be an existing directory you authorize for this document.
Save returns success only after the immutable file generation and selected
commit metadata have been synchronized. A failed Save preserves current text.
The selected canonical directory path, directory identity, exact resource
reference, Body identity, revision, and payload digest support verified
recovery on a fresh owner process. A missing or changed selection is refused;
recovery does not search for another file or invent an unsaved revision.
You can explicitly reselect storage with
`conduit body notebook --state-dir STATE reopen --storage-dir STORAGE`.

For a browser, open a packaged Handbook with `?participate=owner#your-handbook`.
Admit that browser through the owner-authorized window flow in
[the shared owner guide](shared-body-owner.md), while the Body is idle.
Then run `conduit body start --state-dir STATE` to open its bounded editing
encounter, then choose Refresh this Face in the browser. The Face shows Thoughts,
its current text, save status, Replace text,
and Save. Open Plots library exposes the checked `notebook/transition` resident;
Open resident Plot returns to that same document and Play. Open Body overview
returns to its overview. Navigation uses the current acknowledged Show and
never edits or saves the document. Save is available after explicit storage selection. An edit or Save
must name the current Face and Show; an obsolete request cannot overwrite a
newer revision. The current editing encounter has a finite five-minute idle
bound; after it ends, start another encounter to continue. CLI replacements,
Save, and Reopen require an idle Body; `body lull` ends an active encounter.

Run deterministic contracts and actual installed-process acceptance with:

```sh
cargo xtask prove notebook
```

Add `--handbook PACKAGE --output NEW_DIRECTORY` to include actual rendered
edit, Save, stale Show refusal, and replacement browser Host acceptance in the
pinned Chromium. Prepare the verified package through `cargo xtask make body
static --application targets/browser/handbook/handbook.application.template.json
--handbook --output PACKAGE`; browser prerequisites are in
[the browser proof guide](../../proof/browser/README.md).
The proof uses a fresh private installation and records candidate evidence
under ignored output. It does not touch an existing user installation.

Correlated ConduitOS acceptance requires a clean checkout and a native image
and BrowserBundle built from that same commit. First build the canonical guest with
`cargo xtask make conduitos live x86_64 --locked`, then add
`--native-build target/conduitos/live/x86_64-pc` to the browser proof command.
It admits and wears the native Mask while idle, edits in the browser, explicitly
doffs the browser, refreshes ConduitOS through F5, submits a native text edit,
and returns to the browser for Save. Every injected key waits for the guest's
own focus/redraw observation; the driver never retries a semantic action.
The resulting native proof binds the current Face and acknowledged Show to an
independent owner read and checks a browser Show made obsolete by the native edit.
QEMU, `nc`, OpenSSL, a private IPv4 interface, and ordinary Linux file permissions
are required. All invitation secrets and provisioned media stay in a private
fixture directory that the driver removes on completion.

Source-bound final release evidence remains separate from these candidate runs. A browser result alone does not establish guest execution,
physical hardware, automatic replication, or stable publication. The optional
self-editing Source Check milestone is outside this notebook slice.
