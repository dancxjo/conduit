Parent: #5366. Narrowed after second-pass issue granularity audit.

## Outcome
Exercise two accepted Looks under long/Unicode labels, reduced motion, contrast/forced colors and focus with actual source-linked action traversal and safe fallback. Certified accessibility remains owned by #5367.

## Acceptance
- [ ] Meaningful action order and focused item remain reachable after Look swap.
- [ ] Unsupported or unsafe stylistic change refuses or uses trusted fallback.
- [ ] Actual browser/native profile and test evidence correctly classified.

## Migration coordination
One-time Phosphor/ApplicationTheme migration is owned by #5627. #5765 is closed as not planned: no ongoing legacy compatibility tier.

Source, Face/Look/model identity and original #5366 acceptance remain authoritative. Do not duplicate implementation across branches.

## Crèche visual reference (2026-10-20)

Use the user-supplied reference in #6174 as a concrete visual regression specimen: gold focus, muted hints, selected versus unselected Plots, available/unavailable actions, long Unicode, large text and narrow reflow. Actual source state wins over the illustration. Prove current-Show keyboard/pointer reachability, preserved focus/order after Look swap, forced-colors contrast and explicit safe fallback; decorative changes cannot conceal required agency.
