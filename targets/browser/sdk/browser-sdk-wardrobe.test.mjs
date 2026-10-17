import assert from 'node:assert/strict';
import test from 'node:test';
import { checkedOwnerWardrobeAction, checkedOwnerWardrobeReport } from './browser-sdk-wardrobe.mjs';

const plot = suffix => ({ source_document_id: `source/${suffix}`, checked_plot_id: `checked/${suffix}`,
  expanded_plot_id: `expanded/${suffix}` });
const report = {
  schema: 'conduit.body/owner-mask-wardrobe@1', body_id: 'body/clock', owner_plan_id: 'plan/owner',
  wardrobe_revision_decimal: '9007199254740993',
  wardrobe: { revision: 9007199254740992, worn: [plot('browser')], preference: [plot('browser')] },
  admitted_routes: [
    { route_id: 'route/browser', mask_plot: plot('browser'), plan_id: 'plan/owner', currently_available: true },
    { route_id: 'route/terminal', mask_plot: plot('terminal'), plan_id: 'plan/owner', currently_available: false },
  ],
  route_descriptions: [
    { route_id: 'route/browser', mask_name: 'Browser Mask', host_id: 'browser/host' },
    { route_id: 'route/terminal', mask_name: 'Terminal Mask', host_id: 'linux/host' },
  ],
  selected: { route_id: 'route/browser', mask_plot: plot('browser'), plan_id: 'plan/owner' },
  show_id: null, fresh_show_required: true,
};

test('wardrobe preserves exact owner revision and distinguishes route availability from policy', () => {
  const current = checkedOwnerWardrobeReport(report, 'body/clock');
  assert.equal(current.revision, '9007199254740993');
  assert.deepEqual(current.rows.map(row => [row.maskName, row.available, row.worn, row.preferred, row.selected]), [
    ['Browser Mask', true, true, true, true], ['Terminal Mask', false, false, false, false],
  ]);
  assert.deepEqual(checkedOwnerWardrobeAction(report, 'body/clock', 'route/browser', 'doff'), { Doff: plot('browser') });
  assert.deepEqual(checkedOwnerWardrobeAction(report, 'body/clock', 'route/browser', 'prefer'), { Prefer: [plot('browser')] });
  assert.throws(() => checkedOwnerWardrobeAction(report, 'body/clock', 'route/terminal', 'prefer'));
});

test('wardrobe refuses stale identities, invented routes, and malformed selection', () => {
  assert.throws(() => checkedOwnerWardrobeReport(report, 'body/other'));
  assert.throws(() => checkedOwnerWardrobeReport({ ...report, wardrobe_revision_decimal: '9007199254740993.0' }, 'body/clock'));
  assert.throws(() => checkedOwnerWardrobeReport({ ...report, route_descriptions: [] }, 'body/clock'));
  assert.throws(() => checkedOwnerWardrobeReport({ ...report, selected: { ...report.selected, route_id: 'route/ghost' } }, 'body/clock'));
  assert.throws(() => checkedOwnerWardrobeAction(report, 'body/clock', 'route/ghost', 'wear'));
});
