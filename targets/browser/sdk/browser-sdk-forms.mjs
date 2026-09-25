const SOURCE_SCHEMA = "conduit.browser/form-source@1";
const CHECK_SCHEMA = "conduit.browser/checked-source@1";
const FORM_SCHEMA = "conduit.browser/checked-form@1";
const BODY_SCHEMA = "conduit.browser/body@1";
const BODY_KEY = Symbol("Conduit BrowserBody");
let sdkErrors;

export function setBrowserSdkErrors(errors) { sdkErrors = errors; }

export class BrowserForm {
  #source;
  #bridge;
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
  #play = null;
  #opened = false;
  constructor(key, { bridge, host, boot, api, root, createPlay, advertisement, source, receipt, sequence }) {
    if (key !== BODY_KEY) throw new TypeError("BrowserBody values come from an admitted Host BIRTH");
    this.#bridge = bridge; this.#host = host; this.#boot = boot;
    this.#api = api; this.#root = root;
    this.#createPlay = createPlay;
    this.#advertisement = advertisement; this.#source = source;
    this.#receipt = receipt; this.#sequence = sequence;
  }
  get schema() { return BODY_SCHEMA; }
  get id() { return this.#receipt.body_id; }
  get receipt() { return this.#receipt; }

  async current() {
    this.#openWorkspace();
    const { status, outputJson } = this.#bridge.workspaceRequest({ action: "Current" });
    if (status < 0) throw sdkRefusal("Body.current", outputJson, this.#identities());
    return outputJson;
  }

  /** Admit one exact Plan and return the Play receipt emitted by the Rust runtime. */
  async wake() {
    if (this.#play) throw sdkRefusal("Body.wake", { code: "PlayAlreadyActive", message: "Body already has an active Play" }, this.#identities());
    this.#openWorkspace();
    const proposalResult = this.#bridge.workspaceRequest({
      action: "Propose", host_id: this.#host, boot_id: this.#boot,
      source: this.#source, joined_lines: [], browser_audio_authority: false,
    });
    if (proposalResult.status < 0) throw sdkRefusal("Body.wake.propose", proposalResult.outputJson, this.#identities());
    const proposal = proposalResult.outputJson;
    let adapter;
    let playStarted = false;
    try {
      adapter = acquireBrowserBodyHost({
        api: this.#api, hostId: this.#host, bootId: this.#boot, proposal,
        inputTarget: this.#root, outputRoot: this.#root,
        foregroundForm: () => proposal.plan.forms[0]?.form?.checked_form_id ?? proposal.plan.forms[0]?.plan.checked_form_id,
      });
      const started = adapter.start(proposal.wake.wake_sequence);
      playStarted = true;
      const accepted = this.#bridge.workspaceRequest({ action: "Started", host_id: this.#host, boot_id: this.#boot, play: started.play, wake_at_start: started.wake_at_start });
      if (accepted.status < 0) throw sdkRefusal("Body.wake.started", accepted.outputJson, this.#identities());
      const play = this.#createPlay({ started, adapter });
      this.#play = play;
      play.dispatch();
      return play;
    } catch (error) {
      const closed = adapter?.close();
      const refusal = error?.refusal ?? error?.evidence ?? {};
      this.#bridge.workspaceRequest({
        action: "Failed", host_id: this.#host, boot_id: this.#boot,
        rejections: Array.isArray(refusal.rejections) ? refusal.rejections : [],
      });
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
    return this.current();
  }

  async install(form) { return this.#changeWorkset("Install", form); }
  async remove(form) { return this.#changeWorkset("Remove", form); }

  #identities() { return { bodyId: this.id, hostId: this.#host, bootId: this.#boot }; }

  #openWorkspace() {
    if (this.#opened) return;
    const { status, outputJson } = this.#bridge.workspaceRequest({ action: "Arrive", advertisement: this.#advertisement });
    if (status < 0) throw sdkRefusal("Body.open", outputJson, this.#identities());
    this.#opened = true;
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
    return this.current();
  }
}

export async function birthBrowserBody({ bridge, host, boot, api, root, createPlay, membership, name, forms, sequence }) {
  if (!Array.isArray(forms) || forms.length === 0) throw new TypeError("BIRTH requires at least one checked Form");
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
  return new BrowserBody(BODY_KEY, { bridge, host, boot, api, root, createPlay, advertisement: membership.advertisement(), source, receipt: attached.outputJson, sequence });
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

function sdkRefusal(operation, refusal, identity, revision) {
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
