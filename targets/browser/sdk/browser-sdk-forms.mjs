import { createBodyEventStream, maximumBodyEventSubscriptions, projectBodySnapshot } from "./browser-sdk-events.mjs";

const SOURCE_SCHEMA = "conduit.browser/form-source@1";
const CHECK_SCHEMA = "conduit.browser/checked-source@1";
const FORM_SCHEMA = "conduit.browser/checked-form@1";
const BODY_SCHEMA = "conduit.browser/body@1";
const CONTINUITY_SCHEMA = "conduit.browser/body-continuity@1";
const CONTINUITY_KEY = "body-continuity";
const BODY_KEY = Symbol("Conduit BrowserBody");
let sdkErrors;

export function setBrowserSdkErrors(errors) { sdkErrors = errors; }

export class BrowserForm {
  #source;
  #bridge;
  #projectionSequence = 0;
  constructor(source, bridge) {
    if (typeof source !== "string" || source.length === 0) throw new TypeError("Form source must be non-empty Conduitese text");
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
      forms: Object.freeze([]),
      source: this.#source,
      refusal: Object.freeze({ code: outputJson?.code ?? "FormCheckRefused", message: outputJson?.message ?? "Conduit refused the Form source" }),
    });
    const entries = outputJson.forms.map((entry) => Object.freeze({
      schema: FORM_SCHEMA,
      name: entry.name,
      title: entry.title,
      source: entry.source,
      documentSource: this.#source,
      sourceDocumentId: entry.source_document_id,
      checkedFormId: entry.checked_form_id,
      requirements: Object.freeze({ kinds: Object.freeze([...entry.required_kinds]), resources: null, capabilities: null }),
      diagnostics: Object.freeze([]),
      get identity() { return Object.freeze({ sourceDocumentId: this.sourceDocumentId, checkedFormId: this.checkedFormId }); },
    }));
    return Object.freeze({
      schema: CHECK_SCHEMA,
      ok: true,
      sourceDocumentId: outputJson.source_document_id,
      diagnostics: Object.freeze([]),
      requirements: Object.freeze({ kinds: Object.freeze([...new Set(entries.flatMap((entry) => entry.requirements.kinds))]), resources: null, capabilities: null }),
      forms: Object.freeze(entries),
      source: this.#source,
    });
  }

  /** Inspect this authored Form through Rust checking and exact Patchbay projection. */
  patchbay() {
    if (this.#projectionSequence >= Number.MAX_SAFE_INTEGER) {
      throw sdkRefusal("Form.patchbay", {
        code: "ProjectionSequenceExhausted",
        message: "Form Patchbay projection sequence is exhausted",
      });
    }
    const projected = this.#bridge.projectPatchbay(
      this.#source,
      BigInt(++this.#projectionSequence),
    );
    if (projected.status < 0) {
      throw sdkRefusal("Form.patchbay", projected.outputJson);
    }
    return freezePatchbayProjection(projected.outputJson, "Form.patchbay");
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

  async current() {
    await this.#openWorkspace();
    const { status, outputJson } = this.#bridge.workspaceRequest({ action: "Current" });
    if (status < 0) throw sdkRefusal("Body.current", outputJson, this.#identities());
    return projectBodySnapshot(outputJson);
  }

  snapshot() { return this.current(); }

  /**
   * Project this Body's exact checked Form through Rust. The SDK never parses,
   * reconstructs, or renders the topology; a Patchbay workbench Mask consumes
   * the returned immutable value.
   */
  async patchbay() {
    await this.#openWorkspace();
    if (this.#patchbaySequence >= Number.MAX_SAFE_INTEGER) {
      throw sdkRefusal("Body.patchbay", {
        code: "ProjectionSequenceExhausted",
        message: "Patchbay projection sequence is exhausted",
      }, this.#identities());
    }
    const sequence = ++this.#patchbaySequence;
    const current = await this.current();
    const projected = this.#bridge.projectPatchbay(this.#source, BigInt(sequence));
    if (projected.status < 0) {
      throw sdkRefusal("Body.patchbay", projected.outputJson, this.#identities());
    }
    const topology = freezePatchbayProjection(projected.outputJson);
    const resident = current.evidence?.body?.workset?.forms;
    if (!Array.isArray(resident) || !resident.some((form) =>
      form.source_document_id === topology.source_document_id
      && form.checked_form_id === topology.checked_form_id)) {
      throw sdkRefusal("Body.patchbay", {
        code: "PatchbayFormIdentityMismatch",
        message: "Patchbay projection does not match this Body's current checked Form",
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
  async wake() {
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
        inputTarget: this.#root, outputRoot: this.#root,
        foregroundForm: () => proposal.plan.forms[0]?.form?.checked_form_id ?? proposal.plan.forms[0]?.plan.checked_form_id,
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

  async lull() {
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
    return this.current();
  }

  async install(form) { return this.#changeWorkset("Install", form); }
  async remove(form) { return this.#changeWorkset("Remove", form); }

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

  async #changeWorkset(edit, form) {
    const checked = await checkedFormValue(form, this.#bridge);
    if (checked.documentSource !== this.#source) throw new TypeError("Form source must match the Body's checked source document");
    const current = await this.current();
    const expectedRevision = current.evidence.body.workload_revision;
    const { status, outputJson } = this.#bridge.workspaceRequest({
      action: "ChangeWorkset", host_id: this.#host, boot_id: this.#boot,
      expected_revision: expectedRevision,
      form: { source_document_id: checked.sourceDocumentId, checked_form_id: checked.checkedFormId },
      source: this.#source, edit,
    });
    if (status < 0) throw sdkRefusal(`Body.${edit.toLowerCase()}`, outputJson, this.#identities(), expectedRevision);
    await this.#retain();
    return this.current();
  }
}

function freezePatchbayProjection(value, operation = "Body.patchbay") {
  if (value?.schema !== "conduit.patchbay/checked-form-projection@1"
    || typeof value.source_document_id !== "string"
    || typeof value.checked_form_id !== "string"
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

export async function birthBrowserBody({ bridge, host, boot, api, root, createPlay, acquireBodyHost, membership, storage, name, forms, sequence }) {
  if (!Array.isArray(forms) || forms.length === 0) throw new TypeError("BIRTH requires at least one checked Form");
  if (storage && await readContinuity(storage, host, boot) !== null) throw sdkRefusal("Body.birth", {
    code: "RetainedBodyRequiresRecovery",
    message: "this Host retains a Body; recover it before attempting another birth",
  }, { hostId: host, bootId: boot });
  const checked = await Promise.all(forms.map((form) => checkedFormValue(form, bridge)));
  const source = checked[0].documentSource;
  if (checked.some((form) => form.documentSource !== source)) throw new TypeError("initial Forms must come from one exact source document");
  const sourceInteraction = bridge.crecheAdmitSourceInteraction(source, BigInt(sequence()));
  if (sourceInteraction.status < 0) throw sdkRefusal("Body.birth.source", sourceInteraction.outputJson, host);
  const receipt = bridge.crecheBirth({
    host, boot, friendlyName: name,
    initialForms: checked.map(({ name: formName, sourceDocumentId, checkedFormId }) => ({
      name: formName, source_document_id: sourceDocumentId, checked_form_id: checkedFormId,
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
  const checked = await new BrowserForm(retained.source, bridge).check();
  if (!checked.ok) throw sdkRefusal("Host.recover.check", checked.refusal, { hostId: host, bootId: boot });
  const resident = retained.durable.evidence?.body?.workset?.forms;
  if (!Array.isArray(resident) || resident.length < 1 || resident.some(({ source_document_id, checked_form_id }) =>
    !checked.forms.some((form) => form.sourceDocumentId === source_document_id && form.checkedFormId === checked_form_id))) {
    throw sdkRefusal("Host.recover", {
      code: "DurableFormIdentityMismatch",
      message: "retained Body Forms do not match the freshly checked source",
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

export async function reviewBrowserForms({ bridge, host, boot, forms }) {
  if (!Array.isArray(forms) || forms.length === 0) throw new TypeError("Host.review requires checked Forms");
  const checked = await Promise.all(forms.map((form) => checkedFormValue(form, bridge)));
  const source = checked[0].documentSource;
  if (checked.some((form) => form.documentSource !== source)) throw new TypeError("reviewed Forms must come from one exact source document");
  const result = bridge.crecheReviewInitialWorkload({
    host,
    boot,
    source,
    initialForms: checked.map(({ name, sourceDocumentId, checkedFormId }) => ({
      name, source_document_id: sourceDocumentId, checked_form_id: checkedFormId,
    })),
  });
  if (result.status < 0) throw sdkRefusal("Host.review", { ...result.outputJson, code: "PlanRefusal" }, host);
  const reviewReceipt = result.outputJson;
  const review = reviewReceipt.review;
  return Object.freeze({
    schema: "conduit.browser/form-workload-review@1",
    sourceDocumentId: checked[0].sourceDocumentId,
    checkedForms: Object.freeze(checked.map(({ sourceDocumentId, checkedFormId }) => Object.freeze({ sourceDocumentId, checkedFormId }))),
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

async function checkedFormValue(value, bridge) {
  if (value?.schema === SOURCE_SCHEMA && typeof value.source === "string") {
    const checked = await new BrowserForm(value.source, bridge).check();
    if (!checked.ok) throw sdkRefusal("Form.check", checked.refusal);
    if (checked.forms.length !== 1) throw new TypeError("Select one checked Form from the source document before this operation");
    return checked.forms[0];
  }
  if (value?.schema === FORM_SCHEMA && typeof value.source === "string") return value;
  if (value?.schema !== CHECK_SCHEMA || value.ok !== true || !Array.isArray(value.forms)) {
    throw new TypeError("Expected a checked Conduit Form returned by BrowserForm.check()");
  }
  if (value.forms.length !== 1) throw new TypeError("Select one checked Form from the source document before this operation");
  return value.forms[0];
}

export function sdkRefusal(operation, refusal, identity, revision) {
  const code = refusal?.code ?? "FormRefused";
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
