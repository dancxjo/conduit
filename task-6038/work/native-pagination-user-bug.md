## Observed behavior

User-reported normal QEMU Crèche captures show a two-page layout that breaks a related field across both columns and pages:

- Page 1 places “Choose a tradition for the next name suggestion” at the bottom of the left column, while “Use naming tradition” is isolated at the top of the right column.
- The current “Naming tradition: Welsh · Left/Right choose” value is then on page 2, separated from both its prompt and action.
- Most of page 2 is empty. Search and the selected-count summary also appear there, despite substantial unused space across the two captures.

This is a pagination/grouping bug report, rather than evidence that any Source choice was lost or an action mutated incorrectly. Keep headings/help, the action and its current value coherently grouped within the checked structural recipe. Use available space when admitted budgets permit; if pagination is genuinely necessary, preserve meaningful group boundaries and explicit continuation. Do not fix the appearance by increasing the 48-command/192-byte/20-row/eight-surface bounds or changing Source order/authority.

## Original screenshots

![Page 1: naming prompt and action separated across columns](https://raw.githubusercontent.com/dancxjo/conduit/fc18ddb28fcabbad3cb4aac546993b87edc52440/task-6038/outputs/native-pagination-user-report-20261020/page-1.png)

![Page 2: naming value separated onto a mostly empty page](https://raw.githubusercontent.com/dancxjo/conduit/fc18ddb28fcabbad3cb4aac546993b87edc52440/task-6038/outputs/native-pagination-user-report-20261020/page-2.png)

[Byte-preservation receipt](https://github.com/dancxjo/conduit/blob/fc18ddb28fcabbad3cb4aac546993b87edc52440/task-6038/outputs/native-pagination-user-report-20261020/receipt.json). These are the original user attachments, unchanged. The report did not supply an exact Source/product-image identity or guest framebuffer extent; desktop screenshot dimensions are not asserted to be the guest viewport.

## Coordination and acceptance

Related #5990 / #5607, pending panels PR #6055, and parent #5994 / #6038. Inspect the current configured product route and generic fallback before deciding whether #6055 already repairs this exact layout; do not close from a differently sized screenshot alone. Coordinate within the existing native panels/focus work, rather than adding a new serial native milestone.

- [ ] Reproduce or identify the reported Source/recipe/viewport, and exercise the current normal product at the relevant wide extent and a constrained extent.
- [ ] Naming help, action and current value stay coherently grouped; necessary page breaks have honest continuation and do not orphan the field across unrelated columns/pages.
- [ ] Retain complete Source wording/actions/typed arguments, keyboard/pointer reachability and acknowledged Show identity, with no hidden controls or budget increase.
- [ ] Add a deterministic layout regression and retain source/image-pinned normal QEMU before/after evidence. Explain any remaining sparse page that is required by a finite budget rather than claiming all whitespace is avoidable.
