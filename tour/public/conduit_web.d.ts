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
 * Returns a small JSON summary produced from `conduit_panel::parse` itself.
 */
export function parse_panel(source: string): string;

/**
 * Applies one typed candidate transaction against persistent session
 * revisions. Candidate source is resolved and exactly planned before commit.
 */
export function patchbay_apply_transaction(session_id: string, request_json: string): string;

/**
 * Applies a presentation-only visual move through the same Patchbay protocol.
 */
export function patchbay_move_node(source: string, node_id: string, x: number, y: number): string;

/**
 * Opens one finite, revisioned Patchbay authoring session.
 */
export function patchbay_open_session(document_id: string, source: string): string;

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
    readonly parse_panel: (a: number, b: number) => [number, number];
    readonly patchbay_apply_transaction: (a: number, b: number, c: number, d: number) => [number, number];
    readonly patchbay_move_node: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number];
    readonly patchbay_open_session: (a: number, b: number, c: number, d: number) => [number, number];
    readonly patchbay_replace_source: (a: number, b: number, c: number, d: number) => [number, number];
    readonly patchbay_session_view: (a: number, b: number) => [number, number];
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
