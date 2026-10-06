// The browser displays only owner-reported, currently sealed Mask routes.
const MAX_U64 = 18_446_744_073_709_551_615n;
const identity = value => typeof value === 'string' && value.length > 0 && value.length <= 256;
const plot = value => value && typeof value === 'object' &&
  ['source_document_id', 'checked_plot_id', 'expanded_plot_id'].every(key => identity(value[key]));
const samePlot = (left, right) => plot(left) && plot(right) &&
  left.source_document_id === right.source_document_id &&
  left.checked_plot_id === right.checked_plot_id &&
  left.expanded_plot_id === right.expanded_plot_id;

export function checkedOwnerWardrobeReport(report, bodyId) {
  const revision = report?.wardrobe_revision_decimal;
  const wardrobe = report?.wardrobe;
  const routes = report?.admitted_routes;
  const descriptions = report?.route_descriptions;
  if (report?.schema !== 'conduit.body/owner-mask-wardrobe@1' ||
      report.body_id !== bodyId || !identity(report.owner_plan_id) ||
      typeof revision !== 'string' || !/^(0|[1-9][0-9]{0,19})$/.test(revision) ||
      BigInt(revision) > MAX_U64 || !wardrobe || !Array.isArray(wardrobe.worn) ||
      !Array.isArray(wardrobe.preference) || wardrobe.worn.length > 16 ||
      wardrobe.preference.length > 16 || !wardrobe.worn.every(plot) ||
      !wardrobe.preference.every(plot) || !Array.isArray(routes) || !Array.isArray(descriptions) ||
      routes.length !== descriptions.length || routes.length > 8 ||
      !Number.isInteger(wardrobe.revision) || wardrobe.revision < 0 ||
      (BigInt(revision) <= BigInt(Number.MAX_SAFE_INTEGER) && wardrobe.revision !== Number(revision)) ||
      typeof report.fresh_show_required !== 'boolean' ||
      (report.show_id !== null && !identity(report.show_id))) {
    throw new Error('owner wardrobe report is not one bounded current Body report');
  }
  const seen = new Set();
  const rows = routes.map(route => {
    const description = descriptions.find(item => item?.route_id === route?.route_id);
    if (!identity(route?.route_id) || seen.has(route.route_id) || !plot(route.mask_plot) ||
        route.plan_id !== report.owner_plan_id || typeof route.currently_available !== 'boolean' ||
        !description || !identity(description.mask_name) || !identity(description.host_id)) {
      throw new Error('owner wardrobe route lacks exact admitted identity or description');
    }
    seen.add(route.route_id);
    return Object.freeze({
      routeId: route.route_id, maskName: description.mask_name, hostId: description.host_id,
      maskPlot: Object.freeze({ ...route.mask_plot }), available: route.currently_available,
      worn: wardrobe.worn.some(item => samePlot(item, route.mask_plot)),
      preferred: wardrobe.preference.some(item => samePlot(item, route.mask_plot)),
      preferenceRank: wardrobe.preference.findIndex(item => samePlot(item, route.mask_plot)) + 1,
      selected: report.selected?.route_id === route.route_id,
    });
  });
  if (report.selected !== null &&
      (!identity(report.selected?.route_id) || report.selected.plan_id !== report.owner_plan_id ||
       !rows.some(row => row.routeId === report.selected.route_id &&
         samePlot(row.maskPlot, report.selected.mask_plot)))) {
    throw new Error('owner wardrobe selection has no admitted route');
  }
  return Object.freeze({
    ownerPlanId: report.owner_plan_id, revision, rows: Object.freeze(rows),
    preferenceCount: wardrobe.preference.length,
    selectedShowId: report.show_id, freshShowRequired: report.fresh_show_required,
    reconciliation: report.reconciliation, raw: report,
  });
}

export function checkedOwnerWardrobeAction(report, bodyId, routeId, verb) {
  const current = checkedOwnerWardrobeReport(report, bodyId);
  const row = current.rows.find(item => item.routeId === routeId);
  if (!row) throw new Error('Mask route is not in the current owner report');
  if (verb === 'wear' && !row.worn) return Object.freeze({ Wear: row.maskPlot });
  if (verb === 'doff' && row.worn) return Object.freeze({ Doff: row.maskPlot });
  if (verb === 'prefer' && row.worn) return Object.freeze({ Prefer: [row.maskPlot] });
  throw new Error('Mask action is not available for this current route');
}
