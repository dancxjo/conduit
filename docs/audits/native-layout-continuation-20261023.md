# Native continuation pressure: first mechanical-layout seam

Development slice of #6159, supporting #5608/#5994. The existing
[mechanical-layout proposal](https://github.com/dancxjo/conduit/blob/fb335ca25/docs/architecture/mechanical-layout.md)
remains a proposal; no versioned component Fore or general nested/grid engine
is accepted by this change.

The captured ordinary Crèche has 23 content text rows across two pages. Page
one consumes 48 commands while leaving geometric space in another lane. The
existing flat/panel admission accounts for geometry and global resource costs,
but its result did not retain why a continuation was necessary. More pixels
alone cannot admit more commands or rows.

This change retains all active reasons at a page boundary: measured block
shortage, row reservation, command reservation, or failed measurement at the
current geometry. A separate flag says an associated field was moved intact.
A failed width-dependent measurement is deliberately not diagnosed as an
intrinsic width limit: the measurement API does not establish that fact.
Command pressure refers to admitted reserves (including focus and checkbox
state), so actual emitted commands may be below the reserved amount.

The metadata belongs to Mask realization, not Face or Look identity. It is
available in the prepared FaceFrame and ordinary read-only Crèche focus receipt.
The observation is not scanout history. No visible implementation text or new
action is introduced. Geometry/continuation decisions, semantic row/action
order, exact Face bindings and all existing limits remain unchanged. The
independent diagram and document-editor geometry report no row continuation.

Storage uses one niche-packed optional byte per existing prepared row, with no
new collection, cache or candidate loop. Frame reporting reuses its existing
row walk. The existing storage guard passes: Row is 104 bytes on this hosted
x86_64 build, at most 32,768 rows, 3,407,872 bytes of Row storage, and 7,340,032
bytes of conservative indexed-item/row/permutation storage before text. This
is a deterministic storage calculation, not fresh private-guest heap evidence.

## Validation and preserved negatives

- The first compile found a moved measured-demand value. Borrowing it for the
  existing fit check corrected the ownership error; no admission rule changed.
- The scene sweep then ran 118 passing cases, one failed new expectation and
  one existing ignored performance case. The new expectation incorrectly
  assumed a 1024px field specimen had spare block height. Its 18-prefix case
  actually exceeds both block and row capacity; both causes are now required.
- That corrected existing field regression passes separately. All five final
  new pressure/frame cases pass: isolated geometry, isolated row pressure,
  isolated command pressure, simultaneous causes and public-frame exact Face/
  control correspondence. Existing geometry/hit/conformance assertions were
  not relaxed. The broad sweep is not relabelled as a wholly passing rerun.
- The existing preparation-storage case passes with its actual size report.
- Formatting, whitespace and strict all-targets ConduitOS Clippy with
  native-compositor enabled pass. No new lint suppression was added.

[Original logs](evidence/native-layout-continuation-20261023/) retain these
classes, including both negative attempts. No new browser, QEMU, physical,
human, CI or stable-acceptance proof is claimed. Earlier product packets keep
their own Sources. Intrinsic sizing, reusable nested rows/columns/grids/forms,
declared group fragmentation and required wide-view fit remain #6159 work;
#5608's full pressure/provider matrix and the active goal remain incomplete.
