import { createBodyEventStream, maximumBodyEventSubscriptions, projectBodySnapshot } from "./browser-sdk-events.mjs";

const SOURCE_SCHEMA = "conduit.browser/plot-source@1";
const CHECK_SCHEMA = "conduit.browser/checked-source@1";
const PLOT_SCHEMA = "conduit.browser/checked-plot@1";
const BODY_SCHEMA = "conduit.browser/body@1";
const CONTINUITY_SCHEMA = "conduit.browser/body-continuity@1";
const CONTINUITY_KEY = "body-continuity";
const BODY_KEY = Symbol("Conduit BrowserBody");
let sdkErrors;

export function setBrowserSdkErrors(errors) { sdkErrors = errors; }

export class BrowserPlot {
  #source;
  #bridge;
  #projectionSequence = 0;
  constructor(source, bridge) {
    if (typeof source !== "string" || source.length === 0) throw new TypeError("Plot source must be non-empty Conduitese text");
    this.#source = source;
    this.#bridge = bridge;
    Object.freeze(this);
  }
  get schema() { return SOURCE_SCHEMA; }
  get source() { return this.#source; }

  async check() {
    const { status, outputJson } = this.#bridge.crecheReviewedInventory(this.#source);
    if (status < 0) return Object.freeze({
      schema: CHECK_SCHEMA,
      ok: false,
      sourceDocumentId: null,
      diagnostics: Object.freeze((outputJson?.diagnostics ?? []).map((diagnostic) => Object.freeze({ ...diagnostic }))),
      requirements: Object.freeze({ kinds: Object.freeze([]), resources: null, capabilities: null }),
      plots: Object.freeze([]),
      source: this.#source,
      refusal: Object.freeze({ code: outputJson?.code ?? "PlotCheckRefused", message: outputJson?.message ?? "Conduit refused the Plot source" }),
    });
    const entries = outputJson.plots.map((entry) => Object.freeze({
      schema: PLOT_SCHEMA,
      name: entry.name,
      title: entry.title,
      source: entry.source,
      documentSource: this.#source,
      sourceDocumentId: entry.source_document_id,
      checkedPlotId: entry.checked_plot_id,
      requirements: Object.freeze({ kinds: Object.freeze([...entry.required_kinds]), resources: null, capabilities: null }),
      diagnostics: Object.freeze([]),
      get identity() { return Object.freeze({ sourceDocumentId: this.sourceDocumentId, checkedPlotId: this.checkedPlotId }); },
    }));
    return Object.freeze({
      schema: CHECK_SCHEMA,
      ok: true,
      sourceDocumentId: outputJson.source_document_id,
      diagnostics: Object.freeze([]),
      requirements: Object.freeze({ kinds: Object.freeze([...new Set(entries.flatMap((entry) => entry.requirements.kinds))]), resources: null, capabilities: null }),
      plots: Object.freeze(entries),
      source: this.#source,
    });
  }

  /** Inspect this authored Plot through Rust checking and exact Patchbay projection. */
  patchbay() {
    if (this.#projectionSequence >= Number.MAX_SAFE_INTEGER) {
      throw sdkRefusal("Plot.patchbay", {
        code: "ProjectionSequenceExhausted",
        message: "Plot Patchbay projection sequence is exhausted",
      });
    }
    const projected = this.#bridge.projectPatchbay(
      this.#source,
      BigInt(++this.#projectionSequence),
    );
    if (projected.status < 0) {
      throw sdkRefusal("Plot.patchbay", projected.outputJson);
    }
    return freezePatchbayProjection(projected.outputJson, "Plot.patchbay");
  }
}

export class BrowserBody {
  #bridge;
  #host;
  #boot;
  #advertisement;
  #source;
  #receipt;
  #sequence;
  #api;
  #root;
  #createPlay;
  #acquireBodyHost;
  #play = null;
  #opened = false;
  #eventSubscriptions = 0;
  #storage;
  #persistence = Promise.resolve();
  #persistenceFailure = null;
  #patchbaySequence = 0;
  #closing = null;
  #closed = false;
  #operations = Promise.resolve();
  constructor(key, { bridge, host, boot, api, root, createPlay, acquireBodyHost, advertisement, source, receipt, sequence, storage, opened = false }) {
    if (key !== BODY_KEY) throw new TypeError("BrowserBody values come from admitted Host birth or recovery");
    this.#bridge = bridge; this.#host = host; this.#boot = boot;
    this.#api = api; this.#root = root;
    this.#createPlay = createPlay;
    this.#acquireBodyHost = acquireBodyHost;
    this.#advertisement = advertisement; this.#source = source;
    this.#receipt = receipt; this.#sequence = sequence;
    this.#storage = storage;
    this.#opened = opened;
  }
  get schema() { return BODY_SCHEMA; }
  get id() { return this.#receipt.body_id; }
  get receipt() { return this.#receipt; }

  current() { return this.#operate(() => this.#current()); }

  async #current() {
    await this.#openWorkspace();
    const { status, outputJson } = this.#bridge.workspaceRequest({ action: "Current" });
    if (status < 0) throw sdkRefusal("Body.current", outputJson, this.#identities());
    return projectBodySnapshot(outputJson);
  }

  snapshot() { return this.current(); }

  /** Recheck the exact retained source inventory in this runtime. */
  plots() { return this.#operate(() => new BrowserPlot(this.#source, this.#bridge).check()); }

  /**
   * Project this Body's exact checked Plot through Rust. The SDK never parses,
   * reconstructs, or renders the topology; a Patchbay workbench Mask consumes
   * the returned immutable value.
   */
  patchbay() { return this.#operate(() => this.#patchbay()); }

  async #patchbay() {
    await this.#openWorkspace();
    if (this.#patchbaySequence >= Number.MAX_SAFE_INTEGER) {
      throw sdkRefusal("Body.patchbay", {
        code: "ProjectionSequenceExhausted",
        message: "Patchbay projection sequence is exhausted",
      }, this.#identities());
    }
    const sequence = ++this.#patchbaySequence;
    const current = await this.#current();
    const resident = current.evidence?.body?.workset?.plots;
    let projectionSource = this.#source;
    let projectedIdentity = null;
    const bundle = reviewedBundle(this.#source);
    if (bundle) {
      const selected = current.foreground ?? (resident?.length === 1 ? resident[0] : null);
      const inventory = await new BrowserPlot(this.#source, this.#bridge).check();
      const entry = inventory.plots.find(plot => plot.sourceDocumentId === selected?.source_document_id
        && plot.checkedPlotId === selected?.checked_plot_id);
      if (!entry) throw sdkRefusal("Body.patchbay", {
        code: "PatchbayPlotIdentityMismatch",
        message: "Select one resident checked Plot before projecting the bundled Body",
      }, this.#identities());
      projectionSource = entry.source;
      projectedIdentity = selected;
    }
    const projected = this.#bridge.projectPatchbay(projectionSource, BigInt(sequence));
    if (projected.status < 0) {
      throw sdkRefusal("Body.patchbay", projected.outputJson, this.#identities());
    }
    const topology = freezePatchbayProjection(projected.outputJson);
    if ((projectedIdentity && (projectedIdentity.source_document_id !== topology.source_document_id
      || projectedIdentity.checked_plot_id !== topology.checked_plot_id))
      || !Array.isArray(resident) || !resident.some((plot) =>
      plot.source_document_id === topology.source_document_id
      && plot.checked_plot_id === topology.checked_plot_id)) {
      throw sdkRefusal("Body.patchbay", {
        code: "PatchbayPlotIdentityMismatch",
        message: "Patchbay projection does not match this Body's current checked Plot",
      }, this.#identities());
    }
    const snapshot = Object.freeze({
      schema: "conduit.browser/body-patchbay@1",
      bodyId: this.id,
      hostId: this.#host,
      bootId: this.#boot,
      planId: this.#play?.plan.planId ?? null,
      playId: this.#play?.id ?? null,
      topology,
    });
    return snapshot;
  }

  events({ replay = false, pollIntervalMillis = 250, signal } = {}) {
    this.#requireOpen();
    return createBodyEventStream({
      readSnapshot: () => this.current(), signal, replay, pollIntervalMillis,
      reserve: () => {
        if (this.#eventSubscriptions >= maximumBodyEventSubscriptions) {
          throw sdkRefusal("Body.events", { code: "EventPressure", message: "Body event subscription capacity is full" }, this.#identities());
        }
        this.#eventSubscriptions++;
      },
      release: () => { this.#eventSubscriptions = Math.max(0, this.#eventSubscriptions - 1); },
    });
  }

  /** Admit one exact Plan and return the Play receipt emitted by the Rust runtime. */
  wake(options = {}) { return this.#operate(() => this.#wake(options)); }

  async #wake({ root = this.#root, presentationRootFor } = {}) {
    if (this.#play) throw sdkRefusal("Body.wake", { code: "PlayAlreadyActive", message: "Body already has an active Play" }, this.#identities());
    await this.#openWorkspace();
    const proposalResult = this.#bridge.workspaceRequest({
      action: "Propose", host_id: this.#host, boot_id: this.#boot,
      source: this.#source, joined_lines: [], browser_audio_authority: false,
    });
    if (proposalResult.status < 0) throw sdkRefusal("Body.wake.propose", proposalResult.outputJson, this.#identities());
    const proposal = proposalResult.outputJson;
    let adapter;
    let playStarted = false;
    try {
      adapter = this.#acquireBodyHost({
        api: this.#api, hostId: this.#host, bootId: this.#boot, proposal,
        storage: this.#storage,
        inputTarget: root, outputRoot: root, presentationRootFor,
        foregroundPlot: () => {
          const current = this.#bridge.workspaceRequest({ action: "Current" });
          if (current.status < 0) throw sdkRefusal("Body.foreground", current.outputJson, this.#identities());
          return current.outputJson.foreground?.checked_plot_id
            ?? proposal.plan.plots[0]?.plot?.checked_plot_id ?? proposal.plan.plots[0]?.plan.checked_plot_id;
        },
      });
      const started = adapter.start(proposal.wake.wake_sequence);
      playStarted = true;
      const accepted = this.#bridge.workspaceRequest({ action: "Started", host_id: this.#host, boot_id: this.#boot, play: started.play, wake_at_start: started.wake_at_start });
      if (accepted.status < 0) throw sdkRefusal("Body.wake.started", accepted.outputJson, this.#identities());
      await this.#retain();
      const play = this.#createPlay({ started, adapter });
      this.#play = play;
      play.dispatch().catch(() => {});
      return play;
    } catch (error) {
      const closed = adapter?.close();
      const refusal = error?.refusal ?? error?.evidence ?? {};
      this.#bridge.workspaceRequest({
        action: "Failed", host_id: this.#host, boot_id: this.#boot,
        rejections: Array.isArray(refusal.rejections) ? refusal.rejections : [],
      });
      await this.#retain();
      if (playStarted && closed?.receipt) this.#play = null;
      if (error?.category) throw error;
      throw sdkRefusal("Body.wake", { code: error?.refusal?.code ?? error?.code ?? "HostRefusal", message: error?.message ?? "Browser Host refused Play admission", ...(error?.refusal ?? {}) }, this.#identities());
    }
  }

  lull() { return this.#operate(() => this.#lull()); }

  async #lull() {
    if (!this.#play) throw sdkRefusal("Body.lull", { code: "NoActivePlay", message: "Body has no active Play" }, this.#identities());
    const play = this.#play;
    const receipt = await play.terminate();
    const response = this.#bridge.workspaceRequest({
      action: "Lull", host_id: this.#host, boot_id: this.#boot,
      terminated_play: receipt ? play.identity : null,
    });
    if (response.status < 0) throw sdkRefusal("Body.lull", response.outputJson, this.#identities());
    this.#play = null;
    await this.#retain();
    return this.#current();
  }

  /** Select one resident checked Plot through the runtime, retaining its foreground. */
  select(plot) { return this.#operate(() => this.#select(plot)); }

  async #select(plot) {
    const checked = await checkedPlotValue(plot, this.#bridge);
    await this.#openWorkspace();
    const result = this.#bridge.workspaceRequest({ action: "SelectPlot", plot: {
      source_document_id: checked.sourceDocumentId, checked_plot_id: checked.checkedPlotId,
    } });
    if (result.status < 0) throw sdkRefusal("Body.select", result.outputJson, this.#identities());
    await this.#retain();
    return this.#current();
  }

  install(plot) { return this.#operate(() => this.#changeWorkset("Install", plot)); }
  remove(plot) { return this.#operate(() => this.#changeWorkset("Remove", plot)); }
  replace(previous, plot) { return this.#operate(() => this.#changeWorkset("Replace", plot, previous)); }

  /** End execution and settle durable writes before releasing application ownership. */
  close() {
    if (this.#closing) return this.#closing;
    this.#closed = true;
    this.#closing = (async () => {
      await this.#operations;
      if (this.#play) await this.#lull();
      await this.#persistence;
      if (this.#persistenceFailure) throw this.#persistenceFailure;
    })();
    return this.#closing;
  }

  #operate(operation) {
    try { this.#requireOpen(); } catch (error) { return Promise.reject(error); }
    const result = this.#operations.then(operation);
    this.#operations = result.catch(() => {});
    return result;
  }

  #requireOpen() {
    if (this.#closed) throw sdkRefusal("Body.operation", {
      code: "BodyClosed", message: "This Body handle has released its application ownership",
    }, this.#identities());
  }

  #identities() { return { bodyId: this.id, hostId: this.#host, bootId: this.#boot }; }

  async #openWorkspace() {
    if (this.#opened) return;
    const { status, outputJson } = this.#bridge.workspaceRequest({ action: "Arrive", advertisement: this.#advertisement });
    if (status < 0) throw sdkRefusal("Body.open", outputJson, this.#identities());
    this.#opened = true;
    await this.#retain();
  }

  async #retain() {
    if (!this.#storage) return;
    if (this.#persistenceFailure) throw this.#persistenceFailure;
    const { status, outputJson } = this.#bridge.workspaceRequest({ action: "Durable" });
    if (status < 0) throw sdkRefusal("Body.retain", outputJson, this.#identities());
    const record = Object.freeze({ schema: CONTINUITY_SCHEMA, source: this.#source, durable: outputJson });
    this.#persistence = this.#persistence.then(() => this.#storage.writeJson(CONTINUITY_KEY, record));
    try { await this.#persistence; }
    catch (error) {
      this.#persistenceFailure = sdkRefusal("Body.retain", {
        code: error?.code ?? "StorageUnavailable",
        message: error?.message ?? "durable Body continuity could not be retained",
      }, this.#identities());
      throw this.#persistenceFailure;
    }
  }

  async #changeWorkset(edit, plot, previousPlot = null) {
    const checked = await checkedPlotValue(plot, this.#bridge);
    const replaced = previousPlot ? await checkedPlotValue(previousPlot, this.#bridge) : null;
    const current = await this.#current();
    let nextSource = this.#source;
    if (edit !== "Remove" && checked.documentSource !== this.#source) {
      const inventory = await new BrowserPlot(this.#source, this.#bridge).check();
      if (!inventory.ok) throw sdkRefusal("Body.install.source", inventory.refusal, this.#identities());
      const resident = current.evidence.body.workset.plots;
      const entries = inventory.plots.filter(entry => entry.name !== checked.name);
      const previous = inventory.plots.find(entry => entry.name === checked.name);
      if (previous && previous.checkedPlotId !== checked.checkedPlotId
        && previous.checkedPlotId !== replaced?.checkedPlotId
        && resident.some(entry => entry.checked_plot_id === previous.checkedPlotId)) {
        throw sdkRefusal("Body.install", { code: "ResidentPlotConflict",
          message: "Remove the previous checked Plot before installing a changed Plot with the same name" }, this.#identities());
      }
      entries.push(checked);
      nextSource = JSON.stringify({ schema: "conduit.creche/reviewed-plot-bundle@2", plots: entries.map(bundleEntry) });
    }
    const expectedRevision = current.evidence.body.workload_revision;
    const { status, outputJson } = this.#bridge.workspaceRequest({
      action: "ChangeWorkset", host_id: this.#host, boot_id: this.#boot,
      expected_revision: expectedRevision,
      plot: { source_document_id: checked.sourceDocumentId, checked_plot_id: checked.checkedPlotId },
      source: nextSource, edit: replaced ? { Replace: { previous: { source_document_id: replaced.sourceDocumentId, checked_plot_id: replaced.checkedPlotId } } } : edit,
    });
    if (status < 0) throw sdkRefusal(`Body.${edit.toLowerCase()}`, outputJson, this.#identities(), expectedRevision);
    this.#source = nextSource;
    await this.#retain();
    return this.#current();
  }
}

// This only preserves the reviewed envelope; Rust still checks every source and
// presentation profile when the new workset is admitted.
function reviewedBundle(source) {
  try {
    const value = JSON.parse(source);
    return value?.schema === "conduit.creche/reviewed-plot-bundle@2" && Array.isArray(value.plots) ? value : null;
  } catch { return null; }
}

function bundleEntry(plot) {
  const original = reviewedBundle(plot.documentSource)?.plots.find(entry =>
    entry.source === plot.source && (entry.entry ?? entry.slug.replaceAll("-", "_")) === plot.name);
  return original ? { ...original } : {
    slug: plot.name, entry: plot.name, title: plot.title ?? plot.name, source: plot.source,
  };
}

function freezePatchbayProjection(value, operation = "Body.patchbay") {
  if (value?.schema !== "conduit.patchbay/checked-plot-projection@1"
    || typeof value.source_document_id !== "string"
    || typeof value.checked_plot_id !== "string"
    || !Array.isArray(value.front_inputs) || !Array.isArray(value.front_outputs)
    || !Array.isArray(value.gears) || !Array.isArray(value.cords)) {
    throw sdkRefusal(operation, {
      code: "PatchbayProjectionInvalid",
      message: "runtime returned a malformed Patchbay projection",
    });
  }
  const freezePort = (port) => freezePatchbayPort(port, operation);
  return Object.freeze({
    ...value,
    front_inputs: Object.freeze(value.front_inputs.map(freezePort)),
    front_outputs: Object.freeze(value.front_outputs.map(freezePort)),
    gears: Object.freeze(value.gears.map((gear) => Object.freeze({
      ...gear,
      inputs: Object.freeze(gear.inputs.map(freezePort)),
      outputs: Object.freeze(gear.outputs.map(freezePort)),
    }))),
    cords: Object.freeze(value.cords.map((cord) => Object.freeze({ ...cord }))),
    realization_gears: Object.freeze(value.realization_gears.map((gear) => Object.freeze({
      ...gear,
      inputs: Object.freeze(gear.inputs.map(freezePort)),
      outputs: Object.freeze(gear.outputs.map(freezePort)),
    }))),
    realization_cords: Object.freeze(value.realization_cords.map((cord) => Object.freeze({ ...cord }))),
    realization_backs: Object.freeze(value.realization_backs.map((back) => Object.freeze({ ...back }))),
    diagnostics: Object.freeze(value.diagnostics.map((diagnostic) => Object.freeze({
      ...diagnostic,
      subjects: Object.freeze([...diagnostic.subjects]),
    }))),
  });
}

function freezePatchbayPort(port, operation) {
  const contract = port?.value_contract;
  if (!port || typeof port.port_id !== "string" || typeof port.info_kind !== "string"
    || typeof port.temporal !== "string"
    || (contract !== null && (typeof contract !== "object"
      || contract.value_kind !== port.info_kind
      || !Number.isSafeInteger(contract.maximum_bytes) || contract.maximum_bytes < 0
      || !Array.isArray(contract.constraints) || contract.constraints.length > 16))) {
    throw sdkRefusal(operation, {
      code: "PatchbayValueContractInvalid",
      message: "runtime returned a malformed checked Port value contract",
    });
  }
  return Object.freeze({
    ...port,
    value_contract: contract === null ? null : freezeJsonValue(contract),
  });
}

function freezeJsonValue(value) {
  if (Array.isArray(value)) return Object.freeze(value.map(freezeJsonValue));
  if (value && typeof value === "object") {
    return Object.freeze(Object.fromEntries(
      Object.entries(value).map(([key, child]) => [key, freezeJsonValue(child)]),
    ));
  }
  return value;
}

export async function birthBrowserBody({ bridge, host, boot, api, root, createPlay, acquireBodyHost, membership, storage, name, plots, sequence }) {
  if (!Array.isArray(plots) || plots.length === 0) throw new TypeError("BIRTH requires at least one checked Plot");
  if (storage && await readContinuity(storage, host, boot) !== null) throw sdkRefusal("Body.birth", {
    code: "RetainedBodyRequiresRecovery",
    message: "this Host retains a Body; recover it before attempting another birth",
  }, { hostId: host, bootId: boot });
  const checked = await Promise.all(plots.map((plot) => checkedPlotValue(plot, bridge)));
  const source = checked[0].documentSource;
  if (checked.some((plot) => plot.documentSource !== source)) throw new TypeError("initial Plots must come from one exact source document");
  const sourceInteraction = bridge.crecheAdmitSourceInteraction(source, BigInt(sequence()));
  if (sourceInteraction.status < 0) throw sdkRefusal("Body.birth.source", sourceInteraction.outputJson, host);
  const receipt = bridge.crecheBirth({
    host, boot, friendlyName: name,
    initialPlots: checked.map(({ name: plotName, sourceDocumentId, checkedPlotId }) => ({
      name: plotName, source_document_id: sourceDocumentId, checked_plot_id: checkedPlotId,
    })),
    source, sequence: BigInt(sequence()),
  });
  if (receipt.status < 0) throw sdkRefusal("Body.birth", receipt.outputJson, host);
  const attached = bridge.crecheAttachHere(new TextEncoder().encode(host), new TextEncoder().encode(boot), BigInt(sequence()));
  if (attached.status < 0) throw sdkRefusal("Body.attach", attached.outputJson, receipt.outputJson.body_id);
  const body = new BrowserBody(BODY_KEY, { bridge, host, boot, api, root, createPlay, acquireBodyHost, advertisement: membership.advertisement(), source, receipt: attached.outputJson, sequence, storage });
  await body.current();
  return body;
}

export async function recoverBrowserBody({ bridge, host, boot, api, root, createPlay, acquireBodyHost, membership, storage }) {
  if (!storage) throw sdkRefusal("Host.recover", {
    code: "DurabilityDisabled",
    message: "this BrowserHost was admitted without durable continuity",
  }, { hostId: host, bootId: boot });
  const retained = await readContinuity(storage, host, boot);
  if (retained === null) return null;
  if (retained?.schema !== CONTINUITY_SCHEMA || typeof retained.source !== "string"
    || retained.source.length < 1 || retained.durable?.schema !== "conduit.workspace/body@1") {
    throw sdkRefusal("Host.recover", {
      code: "CorruptDurableBody",
      message: "retained Body continuity is malformed or incompatible",
    }, { hostId: host, bootId: boot });
  }
  const checked = await new BrowserPlot(retained.source, bridge).check();
  if (!checked.ok) throw sdkRefusal("Host.recover.check", checked.refusal, { hostId: host, bootId: boot });
  const resident = retained.durable.evidence?.body?.workset?.plots;
  if (!Array.isArray(resident) || resident.length < 1 || resident.some(({ source_document_id, checked_plot_id }) =>
    !checked.plots.some((plot) => plot.sourceDocumentId === source_document_id && plot.checkedPlotId === checked_plot_id))) {
    throw sdkRefusal("Host.recover", {
      code: "DurablePlotIdentityMismatch",
      message: "retained Body Plots do not match the freshly checked source",
    }, { hostId: host, bootId: boot });
  }
  const restored = bridge.workspaceRequest({
    action: "Restore",
    evidence: retained.durable.evidence,
    admission: retained.durable.admission ?? null,
    host_id: host,
    boot_id: boot,
    advertisement: membership.advertisement(),
  });
  if (restored.status < 0) throw sdkRefusal("Host.recover", restored.outputJson, { hostId: host, bootId: boot });
  if (retained.durable.foreground) {
    const selected = bridge.workspaceRequest({ action: "SelectPlot", plot: retained.durable.foreground });
    if (selected.status < 0) throw sdkRefusal("Host.recover.foreground", selected.outputJson, { hostId: host, bootId: boot });
  }
  const evidence = restored.outputJson?.evidence;
  if (!evidence?.body_id) throw sdkRefusal("Host.recover", {
    code: "RecoveryEvidenceMissing",
    message: "runtime recovery omitted the retained Body identity",
  }, { hostId: host, bootId: boot });
  const currentDurable = bridge.workspaceRequest({ action: "Durable" });
  if (currentDurable.status < 0) throw sdkRefusal("Host.recover.retain", currentDurable.outputJson, { bodyId: evidence.body_id, hostId: host, bootId: boot });
  try {
    await storage.writeJson(CONTINUITY_KEY, { schema: CONTINUITY_SCHEMA, source: retained.source, durable: currentDurable.outputJson });
  } catch (error) {
    throw sdkRefusal("Host.recover.retain", {
      code: error?.code ?? "StorageUnavailable",
      message: error?.message ?? "recovered Body continuity could not be retained",
    }, { bodyId: evidence.body_id, hostId: host, bootId: boot });
  }
  const receipt = Object.freeze({
    schema: "conduit.browser/body-recovery@1",
    body_id: evidence.body_id,
    host_id: host,
    boot_id: boot,
    evidence,
  });
  return new BrowserBody(BODY_KEY, {
    bridge, host, boot, api, root, createPlay, acquireBodyHost,
    advertisement: membership.advertisement(), source: retained.source,
    receipt, sequence: () => 0, storage, opened: true,
  });
}

async function readContinuity(storage, host, boot) {
  try { return await storage.readJson(CONTINUITY_KEY); }
  catch (error) {
    throw sdkRefusal("Host.recover.read", {
      code: error?.code ?? "StorageUnavailable",
      message: error?.message ?? "durable Body continuity could not be read",
    }, { hostId: host, bootId: boot });
  }
}

export async function reviewBrowserPlots({ bridge, host, boot, plots }) {
  if (!Array.isArray(plots) || plots.length === 0) throw new TypeError("Host.review requires checked Plots");
  const checked = await Promise.all(plots.map((plot) => checkedPlotValue(plot, bridge)));
  const source = checked[0].documentSource;
  if (checked.some((plot) => plot.documentSource !== source)) throw new TypeError("reviewed Plots must come from one exact source document");
  const result = bridge.crecheReviewInitialWorkload({
    host,
    boot,
    source,
    initialPlots: checked.map(({ name, sourceDocumentId, checkedPlotId }) => ({
      name, source_document_id: sourceDocumentId, checked_plot_id: checkedPlotId,
    })),
  });
  if (result.status < 0) throw sdkRefusal("Host.review", { ...result.outputJson, code: "PlanRefusal" }, host);
  const reviewReceipt = result.outputJson;
  const review = reviewReceipt.review;
  return Object.freeze({
    schema: "conduit.browser/plot-workload-review@1",
    sourceDocumentId: checked[0].sourceDocumentId,
    checkedPlots: Object.freeze(checked.map(({ sourceDocumentId, checkedPlotId }) => Object.freeze({ sourceDocumentId, checkedPlotId }))),
    requirements: Object.freeze({
      kinds: Object.freeze([...reviewReceipt.requirements.kinds]),
      resources: Object.freeze(reviewReceipt.requirements.resources.map((item) => Object.freeze({ hostId: item.host_id, resourceClassId: item.resource_class_id, units: item.units }))),
      capabilities: Object.freeze(reviewReceipt.requirements.capabilities.map((item) => Object.freeze({ hostId: item.host_id, capabilityId: item.capability_id, activeInstances: item.active_instances }))),
    }),
    proposedHosts: Object.freeze(review.proposed_hosts.map((item) => Object.freeze({ hostId: item.host_id, bootId: item.boot_id, profileId: item.profile_id, offerGeneration: item.offer_generation }))),
    reviewedRealizationCount: review.reviewed_realization_count,
    bodyPlanCreated: review.body_plan_created,
    playCreated: review.play_created,
    authorityAcquired: review.authority_acquired,
    resourcesAcquired: review.resources_acquired,
    evidence: reviewReceipt,
  });
}

async function checkedPlotValue(value, bridge) {
  if (value?.schema === SOURCE_SCHEMA && typeof value.source === "string") {
    const checked = await new BrowserPlot(value.source, bridge).check();
    if (!checked.ok) throw sdkRefusal("Plot.check", checked.refusal);
    if (checked.plots.length !== 1) throw new TypeError("Select one checked Plot from the source document before this operation");
    return checked.plots[0];
  }
  if (value?.schema === PLOT_SCHEMA && typeof value.source === "string") return value;
  if (value?.schema !== CHECK_SCHEMA || value.ok !== true || !Array.isArray(value.plots)) {
    throw new TypeError("Expected a checked Conduit Plot returned by BrowserPlot.check()");
  }
  if (value.plots.length !== 1) throw new TypeError("Select one checked Plot from the source document before this operation");
  return value.plots[0];
}

export function sdkRefusal(operation, refusal, identity, revision) {
  const code = refusal?.code ?? "PlotRefused";
  const details = {
    code,
    message: refusal?.message ?? `${operation} was refused`,
    operation,
    evidence: Object.freeze({ ...(refusal ?? {}) }),
    identities: Object.freeze({ ...(identity === undefined ? {} : typeof identity === "string" ? { bodyId: identity } : identity), ...(revision === undefined ? {} : { expectedWorkloadRevision: String(revision) }) }),
  };
  const ErrorType = /Plan|Offer|Unrealizable/.test(code) ? sdkErrors?.PlanRefusalError
    : /ResourceLost|ResourceUnavailable/.test(code) ? sdkErrors?.ResourceLossError
      : /PermissionDenied|PermissionRefused/.test(code) ? sdkErrors?.PermissionDeniedError
        : /IncompatibleRuntimeAbi/.test(code) ? sdkErrors?.IncompatibleRuntimeAbiError
          : sdkErrors?.InvalidLifecycleError;
  if (ErrorType) return new ErrorType(details);
  const error = new Error(details.message);
  error.name = "InvalidLifecycleError";
  Object.assign(error, details, { category: "InvalidLifecycle" });
  return error;
}
