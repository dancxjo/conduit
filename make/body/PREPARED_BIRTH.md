# Reviewed Crèche birth for image preparation

`PreparedBodyBirth::from_reviewed_draft` consumes the owning Crèche draft's
reviewed revision and an attributable birth Sign. It asks the existing draft
for its selection, then uses ordinary Body birth, membership and biography.
An obsolete revision or unfinished name refuses before birth. User-edited
names and the exact selected workset are retained; make does not restart naming.

Prepare once for the new Body. All selected target builders consume that same
biography alongside their own exact Host provisioning. Building several Host
artifacts must not perform an independent birth for each target. Separate new
Bodies require separate attributable birth Signs even when names match.

The resulting Body is Lulled with empty membership and no current realization.
An image does not grant resources or invent a future Boot. The deployed Host
must initialize real resources, admit its actual identity and fresh Boot, and
use the ordinary lifecycle for Wake/Plan/Play.

This is a shared prerequisite for #6458 and the appliance producer in #6457.
It does not yet wire either producer, manufacture firmware, execute a Crèche
Plot, or establish native/physical deployment proof. The appliance producer
can delegate its default generated-name draft through this entrance; the
interactive producer passes the actual edited and reviewed draft.

The repository foundation suite (`cargo xtask check workspace-test-foundation`)
includes this package's contract tests. Construction fixtures verify edited
selection retention through biography serialization/reopening, stale-review
refusal, invalid-name refusal and independent birth identities.
