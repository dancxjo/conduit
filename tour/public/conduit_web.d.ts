/* tslint:disable */
/* eslint-disable */

/**
 * Starts the production executor and applies deterministic abort
 * cancellation before its first node step.
 */
export function cancel_panel(source: string): string;

/**
 * Returns the production resolver's logical and expanded projections.
 */
export function explain_panel(source: string): string;

/**
 * Returns parser-owned lexical metadata for browser source presentation.
 */
export function panel_language_metadata(): string;

/**
 * Returns exact semantic port ranges derived from the production parser.
 *
 * Malformed source deliberately returns no semantic annotations: the browser
 * may retain lossless lexical presentation, but it must not guess direction.
 */
export function panel_source_metadata(source: string): string;

/**
 * Returns a small JSON summary produced from `conduit_panel::parse` itself.
 */
export function parse_panel(source: string): string;

/**
 * Advances deterministic browser-host time to an exact pending deadline.
 * It is an explicit host wake, not a JavaScript executor or clock jump.
 */
export function patchbay_advance_exact_run(session_id: string, run_id: string, source_revision: bigint, plan_identity: string, tick: bigint): string;

/**
 * Applies one typed candidate transaction against persistent session
 * revisions. Candidate source is resolved and exactly planned before commit.
 */
export function patchbay_apply_transaction(session_id: string, request_json: string): string;

/**
 * Attaches one slot already admitted by the active exact plan. This changes
 * observation control only; source and plan identities remain pinned.
 */
export function patchbay_attach_exact_watch(session_id: string, run_id: string, source_revision: bigint, plan_identity: string, operator_id: string, watch_id: string): string;

/**
 * Requests the exact plan-visible stop disposition for the active browser
 * run. `drain` and `abort` stay distinct through the shared runtime session.
 */
export function patchbay_cancel_exact_run(session_id: string, run_id: string, source_revision: bigint, plan_identity: string, disposition: string): string;

/**
 * Detaches one active Watch while preserving its bounded retained window.
 */
export function patchbay_detach_exact_watch(session_id: string, run_id: string, source_revision: bigint, plan_identity: string, operator_id: string, watch_id: string): string;

/**
 * Releases one terminal exact run while retaining its authoring workspace.
 * Live, waiting, quiescing, and aborting runs must first reach a terminal
 * state through the production executor.
 */
export function patchbay_dispose_exact_run(session_id: string, run_id: string, source_revision: bigint, plan_identity: string): string;

/**
 * Applies a presentation-only visual move through the same Patchbay protocol.
 */
export function patchbay_move_node(source: string, node_id: string, x: number, y: number): string;

/**
 * Delivers one exact named host-operation wake to the browser-owned session.
 * The supplied subject is validated but never retained by the bridge; only an
 * already registered exact wait can become runnable.
 */
export function patchbay_notify_host_operation(session_id: string, run_id: string, source_revision: bigint, plan_identity: string, subject: string): string;

/**
 * Opens one finite, revisioned Patchbay authoring session.
 */
export function patchbay_open_session(document_id: string, source: string): string;

/**
 * Runs one bounded cooperative turn of the active browser-worker exact run.
 */
export function patchbay_pump_exact_run(session_id: string, run_id: string, source_revision: bigint, plan_identity: string, quantum: bigint): string;

/**
 * Returns one bounded, read-only delta from the worker-owned committed
 * evidence provider. Patchbay never acknowledges or releases scheduler
 * storage and therefore cannot become the authoritative evidence store.
 */
export function patchbay_read_exact_evidence(session_id: string, run_id: string, source_revision: bigint, plan_identity: string, cursor: bigint, maximum_events: number): string;

/**
 * Reads one bounded Watch delta from a live session or its final retained
 * window. Binary bytes remain bytes; only the exact `std/text`
 * representation receives a UTF-8 text projection.
 */
export function patchbay_read_exact_watch(session_id: string, run_id: string, source_revision: bigint, plan_identity: string, operator_id: string, watch_id: string, cursor: bigint, maximum_records: number): string;

/**
 * Applies a source transaction through the production Patchbay protocol.
 * The browser receives only the separate source/semantic/presentation facts.
 */
export function patchbay_replace_source(source: string, replacement: string): string;

/**
 * Returns the current authoritative Rust projection for a Patchbay session.
 */
export function patchbay_session_view(session_id: string): string;

/**
 * Returns one bounded authoritative recovery snapshot for an exact run.
 * Callers use this after an evidence or Watch cursor gap; the snapshot names
 * the retained cursor windows but does not replay unbounded history.
 */
export function patchbay_snapshot_exact_run(session_id: string, run_id: string, source_revision: bigint, plan_identity: string): string;

/**
 * Explicitly starts one browser-worker exact run from the current source
 * revision. This is the only operation that may create a new run epoch;
 * authoring and checking remain non-actuating.
 */
export function patchbay_start_exact_run(session_id: string): string;

/**
 * Observes compiled-in browser providers and executes their exact plan.
 */
export function run_panel(source: string): string;

/**
 * Executes the finite hosted compatibility demo with bounded in-memory streams.
 */
export function run_panel_compatibility_demo(source: string): string;

/**
 * Compiles immutable inputs and executes their exact plan through the same
 * bounded deterministic executor used by `conduct run`.
 */
export function run_panel_exact(source: string, compile_input_json: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly cancel_panel: (a: number, b: number) => [number, number];
    readonly explain_panel: (a: number, b: number) => [number, number];
    readonly panel_language_metadata: () => [number, number];
    readonly panel_source_metadata: (a: number, b: number) => [number, number];
    readonly parse_panel: (a: number, b: number) => [number, number];
    readonly patchbay_advance_exact_run: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: number, h: bigint) => [number, number];
    readonly patchbay_apply_transaction: (a: number, b: number, c: number, d: number) => [number, number];
    readonly patchbay_attach_exact_watch: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: number, h: number, i: number, j: number, k: number) => [number, number];
    readonly patchbay_cancel_exact_run: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: number, h: number, i: number) => [number, number];
    readonly patchbay_detach_exact_watch: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: number, h: number, i: number, j: number, k: number) => [number, number];
    readonly patchbay_dispose_exact_run: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: number) => [number, number];
    readonly patchbay_move_node: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number];
    readonly patchbay_notify_host_operation: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: number, h: number, i: number) => [number, number];
    readonly patchbay_open_session: (a: number, b: number, c: number, d: number) => [number, number];
    readonly patchbay_pump_exact_run: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: number, h: bigint) => [number, number];
    readonly patchbay_read_exact_evidence: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: number, h: bigint, i: number) => [number, number];
    readonly patchbay_read_exact_watch: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: number, h: number, i: number, j: number, k: number, l: bigint, m: number) => [number, number];
    readonly patchbay_replace_source: (a: number, b: number, c: number, d: number) => [number, number];
    readonly patchbay_session_view: (a: number, b: number) => [number, number];
    readonly patchbay_snapshot_exact_run: (a: number, b: number, c: number, d: number, e: bigint, f: number, g: number) => [number, number];
    readonly patchbay_start_exact_run: (a: number, b: number) => [number, number];
    readonly run_panel: (a: number, b: number) => [number, number];
    readonly run_panel_compatibility_demo: (a: number, b: number) => [number, number];
    readonly run_panel_exact: (a: number, b: number, c: number, d: number) => [number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
