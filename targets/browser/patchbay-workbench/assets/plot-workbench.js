// Maker-facing chrome over the current canonical snapshot. No graph or edit state lives here.
function fact(face, subject, name) {
  const value = face.properties.find(item => item.subject === subject && item.name === name)?.value;
  return value?.Identity ?? value?.Text ?? value?.Count ?? value?.Flag ?? null;
}
export function openedPlot(snapshot) {
  return snapshot.face.subjects.find(subject => subject.role === "Plot"
    && fact(snapshot.face, subject.identity, "opened") === true) ?? null;
}
export function selectionTitle(snapshot, subject) {
  return subject.role === "Play" && fact(snapshot.face, subject.identity, "play-state")
    ? "Execution result" : subject.name;
}
export function selectionFacts(snapshot, subject) {
  const face = snapshot.face;
  if (subject.role === "Play" && fact(face, subject.identity, "play-state")) return [["State", fact(face, subject.identity, "play-state")], ["Execution Signs", fact(face, subject.identity, "sign-count")]];
  if (!snapshot.authoring || snapshot.navigation?.cursor.aspect !== "Structure") return null;
  if (subject.role === "Gear") {
    const kind = fact(face, subject.identity, "kind-id");
    const entry = snapshot.authoring.palette.find(item => item.kind_id === kind);
    const ports = face.relationships.filter(item => item.kind === "Contains" && item.source === subject.identity)
      .map(item => face.subjects.find(candidate => candidate.identity === item.target))
      .filter(item => item?.role === "Port");
    return [["Gear", subject.name], ["Kind", kind], ...(entry ? [["Purpose", entry.summary]] : []),
      ...ports.map(port => [fact(face, port.identity, "direction") === "receiving" ? "Input" : "Output",
        `${fact(face, port.identity, "port-id")} · ${fact(face, port.identity, "value-kind")} · ${fact(face, port.identity, "temporal")}`])];
  }
  if (subject.role === "Plot") return [["Plot", subject.name], ["State", "Editing does not start a Body."]];
  if (subject.role === "Port") return [["Port", fact(face, subject.identity, "port-id")],
    ["Direction", fact(face, subject.identity, "direction")], ["Carries", fact(face, subject.identity, "value-kind")],
    ["Timing", fact(face, subject.identity, "temporal")]];
  if (subject.role === "Cord") {
    const port = name => {
      const identity = fact(face, subject.identity, name);
      return face.subjects.find(item => item.role === "Port" && fact(face, item.identity, "semantic-id") === identity);
    };
    const endpoint = item => {
      if (!item) return "Endpoint unavailable in this Face";
      const owner = face.relationships.find(relation => relation.kind === "Contains" && relation.target === item.identity);
      const gear = face.subjects.find(candidate => candidate.identity === owner?.source);
      return `${gear?.name ?? "Plot"} · ${fact(face, item.identity, "port-id")}`;
    };
    return [["From", endpoint(port("source-port"))], ["To", endpoint(port("sink-port"))],
      ["Carries", fact(face, subject.identity, "value-kind")],
      ["Direction", "Output → input"]];
  }
  return null;
}

export function installPlotWorkbench({ open, invoke, navigate, inspect }) {
  const heading = document.querySelector(".canvas-heading");
  const toolbar = document.createElement("div");
  toolbar.id = "maker-toolbar";
  toolbar.setAttribute("aria-label", "Current Plot actions");
  heading.append(toolbar);
  const invitation = document.createElement("section");
  invitation.id = "maker-invitation";
  invitation.setAttribute("aria-label", "Begin composing");
  const title = document.createElement("h2"); title.textContent = "Make the system you mean";
  const explanation = document.createElement("p");
  explanation.textContent = "Open a Plot to explore its Gears and connections. Use the Gear Library to make it your own.";
  const choices = document.createElement("div"); choices.id = "maker-choices";
  const boundary = document.createElement("p"); boundary.textContent = "Editing a Plot does not start a Body.";
  invitation.append(title, explanation, choices, boundary);
  document.querySelector("#plot").append(invitation);
  const status = document.createElement("output");
  status.id = "maker-status"; status.setAttribute("role", "status");
  document.querySelector("#patchbay-root").append(status);
  const controls = new Map();
  const views = document.createElement("details"); views.id = "maker-views";
  const viewsLabel = document.createElement("summary"); viewsLabel.textContent = "Views and diagnostics";
  views.append(viewsLabel);
  new ResizeObserver(() => document.body.style.setProperty("--maker-heading-bottom", `${Math.ceil(heading.getBoundingClientRect().bottom + 8)}px`)).observe(heading);
  let current = null, pending = false, feedback = null, priorInteraction = null, makerEncounter = false;
  function button(parent, key, label, run, unavailable = null) {
    let control = controls.get(key);
    if (!control) {
      control = document.createElement("button"); control.type = "button";
      control.dataset.makerAction = key; controls.set(key, control); parent.append(control);
    }
    control.textContent = label; control.disabled = pending || unavailable !== null;
    control.title = unavailable ?? "";
    control.onclick = async () => {
      pending = true; feedback = null; control.disabled = true;
      status.textContent = `${label} pending…`;
      try {
        const next = await run();
        if (next?.interaction && next.interaction.last_disposition !== "Succeeded") throw new Error(next.interaction.last_disposition ?? "No canonical acknowledgement received");
      } catch (error) { feedback = error.message; }
      finally { pending = false; if (current) render(current); }
    };
    return control;
  }
  function render(snapshot) {
    current = snapshot;
    makerEncounter ||= snapshot.face.basis.body_id === null || !!snapshot.authoring;
    if (!makerEncounter) {
      document.body.dataset.makerMode = "legacy";
      invitation.hidden = true; toolbar.hidden = true; status.hidden = true;
      return;
    }
    const entrance = snapshot.face.basis.body_id === null && snapshot.navigation?.cursor.place === "Entrance";
    const editing = !!snapshot.authoring && !entrance;
    document.body.dataset.makerMode = entrance ? "welcome" : editing ? "edit" : snapshot.body_workbench ? "body" : "run";
    if ((entrance || editing) && !views.isConnected) {
      views.append(document.querySelector("#lens-label"), document.querySelector(".view-controls"), document.querySelector("#current-status"), document.querySelector("#front-door-actions"), document.querySelector("#temporal-context"));
      heading.firstElementChild.append(views);
    }
    invitation.hidden = !entrance;
    toolbar.hidden = entrance;
    status.hidden = !!snapshot.body_workbench;
    const interaction = snapshot.interaction;
    if (interaction.revision !== priorInteraction) {
      priorInteraction = interaction.revision;
      feedback = interaction.last_disposition?.startsWith("Refused") ? interaction.last_disposition : null;
    }
    const plot = openedPlot(snapshot);
    if (entrance) {
      const retained = new Set();
      for (const subject of snapshot.face.subjects.filter(subject => subject.role === "Plot")) {
        const action = snapshot.face.actions.find(action => action.target === subject.identity && action.intent === "conduit.intent/open@1");
        if (!action) continue;
        const key = `open-${subject.identity}`; retained.add(key);
        button(choices, key, `Explore ${subject.name}`, () => open(subject.identity));
      }
      for (const [key, control] of controls) if (key.startsWith("open-") && !retained.has(key)) { control.remove(); controls.delete(key); }
      document.querySelector("#canvas-title").textContent = "Your Plot workbench";
    } else if (editing && plot) {
      document.querySelector("#canvas-title").textContent = plot.name;
    }
    const retained = new Set();
    const add = (key, label, run, unavailable) => { retained.add(key); button(toolbar, key, label, run, unavailable); };
    if (editing) {
      add("plots", "Open another Plot", async () => {
        const search = document.querySelector("#library-query");
        search.value = "";
        search.dispatchEvent(new Event("input", {bubbles:true}));
        await navigate({kind:"enter",place:"Entrance"});
        document.body.dataset.paletteOpen = "true";
        document.querySelector("#toggle-palette").setAttribute("aria-expanded", "true");
      });
      if (plot) add("inspect", "Inspect Plot", () => inspect(plot.identity));
    }
    const basis = snapshot.face.basis;
    const play = snapshot.face.subjects.find(subject => subject.role === "Play");
    const terminal = play && fact(snapshot.face, play.identity, "play-state");
    const signCount = play && fact(snapshot.face,play.identity,"sign-count");
    if (!entrance && !editing && snapshot.navigation?.navigation.places.some(place => place.place === "Program")) {
      add("program", "Show Plot", () => navigate({kind:"enter",place:"Program"}));
    }
    if (play && terminal) add("result", "Inspect execution", async () => {
      await navigate({kind:"show",aspect:"Play"});
      return inspect(play.identity);
    });
    const intents = editing ? [["save", "Save Plot"], ["birth", "Birth Body"]]
      : !basis.wake_id ? [["wake", "Wake Body"]] : !basis.plan_id ? [["plan", "Plan Plot"]]
      : !basis.active_play_id ? [["play", "Play Plot"]] : [];
    for (const [intent, label] of intents) {
      const action = snapshot.face.actions.find(action => action.intent === `conduit.intent/${intent}@1`
        && (editing ? action.target === plot?.identity : true));
      if (!action) continue;
      const unavailable = action.availability === "Available" ? null
        : (action.availability?.Unavailable ?? action.availability?.Refused)?.explanation ?? "Unavailable";
      const basis = {face_id:snapshot.face.identity,face_revision:snapshot.face.revision};
      add(intent, label, () => invoke(action, basis), unavailable);
    }
    for (const [key, control] of controls) if (!key.startsWith("open-") && !retained.has(key)) { control.remove(); controls.delete(key); }
    const source = snapshot.authoring;
    status.textContent = feedback ?? (entrance ? "Choose a Plot to begin. No Body is running."
      : editing ? source.source_revision !== source.saved_revision ? "Not running. Unsaved Plot changes." : "Not running. Plot source is saved. You can edit this Plot."
      : terminal ? `Play ${String(terminal).toLowerCase()} · ${signCount} execution Sign${signCount === 1 ? "" : "s"}. Inspect execution for the exact evidence.`
      : basis.active_play_id ? "Play active." : basis.plan_id ? "Plan ready. Choose Play Plot to execute."
      : basis.wake_id ? "Body awake. Choose Plan Plot to prepare execution." : "Body created. Wake is explicit.");
    status.dataset.refused = String(feedback !== null);
  }
  return { render, notice(text, component) {
    if (!makerEncounter) return;
    if (component === "success-status") { feedback = null; if (current) render(current); }
    if (/failure|failed|refused/.test(component)) { feedback = text; status.textContent = text; status.dataset.refused = "true"; status.hidden = false; }
  }};
}
