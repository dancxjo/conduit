/** Read-only projection of the exact execution proposal, never a planner or registry. */
export function presentBodyPlan(root, proposal) {
  const document = root.ownerDocument;
  const candidate = document.createDocumentFragment();
  const paragraph = (parent, text) => {
    const node = document.createElement("p");
    node.textContent = text;parent.append(node);
  };
  const details = (parent, title) => {
    const node = document.createElement("details"), summary = document.createElement("summary");
    summary.textContent = title;node.append(summary);parent.append(node);
    return node;
  };
  const plan = proposal.plan;
  paragraph(candidate, `Selected Body Plan ${plan.plan_id} · Body ${plan.body_id} · Wake ${plan.wake_id}`);
  paragraph(candidate, "Retained selection, not current availability or physical proof. Device and Base associations are not recorded in this proposal; absence here does not prove no Device exists.");
  for (const form of plan.forms) {
    const formNode = details(candidate, `Form ${form.plan.checked_form_id}`);
    paragraph(formNode, `Source ${form.plan.source_document_id} · Form Plan ${form.plan.plan_id}`);
    for (const fragment of form.plan.fragments) {
      for (const placement of fragment.placements) {
        const gear = details(formNode, `Gear ${placement.gear_id} · ${placement.kind_id}`);
        gear.dataset.placementId = placement.placement_id;
        paragraph(gear, `Placement ${placement.placement_id} · Offer ${placement.capability_id}`);
        paragraph(gear, `Host ${placement.host_id} · Boot ${placement.boot_id} · offer generation ${placement.offer_generation}`);
        paragraph(gear, `Implementation ${placement.implementation_id} · Artifact ${placement.artifact_id}`);
        for (const direction of ["inputs", "outputs"]) {
          for (const port of placement[direction]) paragraph(gear, `${direction}: ${JSON.stringify(port)}`);
        }
        paragraph(gear, `Resources: ${JSON.stringify(placement.resources)}`);
        paragraph(gear, `Host operations: ${JSON.stringify(placement.host_operations)}`);
        paragraph(gear, `Authority: ${JSON.stringify(placement.authority)}`);
      }
      const cords = details(formNode, `Cords · fragment ${fragment.fragment_id}`);
      for (const connection of fragment.connections) paragraph(cords, JSON.stringify(connection));
    }
  }
  root.replaceChildren(candidate);
}
