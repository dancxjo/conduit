import { fixtureState } from "./state.mjs";
import { productMastheadNodes } from "./product-masthead.mjs";

const decoder = new TextDecoder();

export async function startApplication(context) {
  if (context.schema !== "conduit.browser/application-context@1") throw new Error("fourth application context is unsupported");
  const state = {
    revision: 0,
    destination: "overview",
    name: "Ada",
    status: "Ready",
    disposition: "missing-evidence",
    evidence: "No operation has run.",
    progress: "1/3",
  };

  const description = () => ({
    actions: [
      { id: "navigate.overview", event: "activate" },
      { id: "navigate.work", event: "activate" },
      { id: "name.input", event: "input" },
      { id: "prove.success", event: "activate" },
      { id: "prove.stale", event: "activate" },
      { id: "prove.unsupported", event: "activate" },
      { id: "prove.host-failure", event: "activate" },
      { id: "prove.pressure", event: "activate" },
    ],
    nodes: [
      { parent: null, component: "shell", key: "shell", text: "", action: null },
      ...productMastheadNodes({ parent: 0, firstIndex: 1, status: state.status }),
      { parent: 0, component: "main", key: "main", text: "", action: null },
      { parent: 9, component: "heading", key: "title", text: fixtureState.title, action: null },
      { parent: 9, component: "paragraph", key: "summary", text: fixtureState.summary, action: null },
      { parent: 9, component: "navigation", key: "navigation", text: "Field Notes destinations", value: state.destination, valueCapacity: 32, action: null },
      { parent: 12, component: "button", key: "overview", text: "Overview", action: 0 },
      { parent: 12, component: "button", key: "work", text: "Work", action: 1 },
      { parent: 9, component: "status", key: "status", text: state.status, action: null },
      { parent: 9, component: "form-field", key: "name-field", text: "", action: null },
      { parent: 16, component: "field-label", key: "name-label", text: "Observer name", action: null },
      { parent: 16, component: "text-input", key: "name", text: "Observer name", value: state.name, valueCapacity: 16, action: 2 },
      { parent: 16, component: "field-help", key: "name-help", text: "At most 16 UTF-8 bytes.", action: null },
      { parent: 9, component: "action-group", key: "actions", text: "Proof actions", action: null },
      { parent: 20, component: "button", key: "success", text: "Record success", action: 3 },
      { parent: 20, component: "button", key: "stale", text: "Prove stale revision", action: 4 },
      { parent: 20, component: "button", key: "unsupported", text: "Prove unsupported mechanism", action: 5 },
      { parent: 20, component: "button", key: "host-failure", text: "Prove Host-effect failure", action: 6 },
      { parent: 20, component: "button", key: "pressure", text: "Hold one pressure event", action: 7 },
      { parent: 20, component: "button", state: "unavailable", key: "unavailable", text: "Unavailable action", action: null },
      { parent: 9, component: "progress", key: "progress", text: "Proof progress", value: state.progress, valueCapacity: 11, action: null },
      { parent: 9, component: "artifact", key: "artifact", text: fixtureState.artifact, action: null },
      { parent: 28, component: "paragraph", key: "artifact-summary", text: `Observed by ${state.name}`, action: null },
      { parent: 28, component: "disclosure", key: "disclosure", text: "Exact observation", action: null },
      { parent: 30, component: "code-block", key: "exact", text: "text", value: fixtureState.exactEvidence, valueCapacity: 256, action: null },
      { parent: 9, component: state.disposition, key: "evidence", text: "Current evidence", action: null },
      { parent: 32, component: "paragraph", key: "evidence-detail", text: state.evidence, action: null },
    ],
  });

  const render = () => context.presentation.present("application", {
    revision: ++state.revision,
    ...description(),
  }, {
    eventCapacity: 1,
    async onEvent(event) {
      if (event.action === "prove.pressure") return;
      context.presentation.nextEvent("application");
      if (event.action === "navigate.overview" || event.action === "navigate.work") {
        state.destination = event.action.slice("navigate.".length);
        state.status = `Opened ${state.destination}`;
      } else if (event.action === "name.input") {
        state.name = decoder.decode(event.value);
        state.status = "Name accepted";
      } else if (event.action === "prove.success") {
        state.disposition = "successful-evidence";
        state.evidence = "Success remained distinct and bounded.";
        state.progress = "2/3";
      } else if (event.action === "prove.stale") {
        try { context.presentation.present("application", { revision: state.revision, ...description() }); }
        catch (error) {
          state.disposition = "stale-evidence";
          state.evidence = error.code;
        }
      } else if (event.action === "prove.unsupported") {
        try {
          context.presentation.present("application", {
            revision: state.revision + 1,
            actions: [],
            nodes: [{ parent: null, component: "future-widget", key: "future", text: "", action: null }],
          });
        } catch (error) {
          state.disposition = "refused-evidence";
          state.evidence = error.code;
        }
      } else if (event.action === "prove.host-failure") {
        state.status = "Host effect pending";
        render();
        try { await context.storage.writeJson("oversized", "x".repeat(context.storage.bounds.maximumApplicationBytes + 1)); }
        catch (error) {
          state.disposition = "failed-evidence";
          state.evidence = error.code;
          state.status = "Host effect failed";
          state.progress = "3/3";
          render();
        }
        return;
      }
      render();
    },
    onRefusal(code) {
      state.disposition = code === "stale-revision" ? "stale-evidence" : "refused-evidence";
      state.evidence = code;
      state.status = "Action refused";
      render();
    },
  });

  render();
}
