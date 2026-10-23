const PREFIX = "CONDUIT_HOST_STARTUP_PHASE ";
const MAX_BYTES = 16 * 1024, MAX_RECORDS = 40, MAX_LINE = 512;
const PHASES = [
  "service-lock", "body-lock", "retained-recovery", "ownership-selection", "retained-scan",
  "installation", "boot-identity", "host-construction", "equipment-selection", "runtime-identity",
  "image-digest", "runtime-state", "catalogs", "birth-encounter", "runtime-publication", "owned-body",
];
const OPTIONAL = new Set(["birth-encounter", "owned-body"]);
const KEYS = ["schema", "pid", "sequence", "event", "phase", "elapsed_us", "duration_us"];
const integer = value => Number.isSafeInteger(value) && value >= 0;

// The caller finishes this recorder only after stopProcess has drained the owned pipes.
// A forced cleanup failure yields an explicit partial prefix, never inferred readiness.
export function recordNativeStartup(child) {
  const bytes = Buffer.alloc(MAX_BYTES);
  let used = 0, truncated = false, result;
  const inspect = chunk => {
    const input = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
    const copied = input.copy(bytes, used, 0, Math.min(input.length, MAX_BYTES - used));
    used += copied; truncated ||= copied < input.length;
  };
  child.stderr.on("data", inspect);
  return () => {
    if (result) return result;
    child.stderr.off("data", inspect);
    result = parseNativeStartup(bytes.subarray(0, used), child.pid ?? null, truncated);
    return result;
  };
}

export function parseNativeStartup(bytes, pid, truncated = false) {
  const record = { label: "native startup", pid, outcome: "absent", records: [],
    output_truncated: truncated, incomplete_line: false, active_phase: null };
  if (bytes.length > MAX_BYTES) {
    bytes = bytes.subarray(0, MAX_BYTES); record.output_truncated = true;
  }
  let cursor = 0, started = false, terminal = false, failed = false, previous = 0, lastPhase = -1;
  const seen = new Set();
  const refuse = reason => { record.outcome = "refused"; record.error = reason; return record; };
  while (cursor < bytes.length) {
    const end = bytes.indexOf(10, cursor);
    if (end < 0) { record.incomplete_line = true; break; }
    const line = bytes.subarray(cursor, end); cursor = end + 1;
    if (!line.subarray(0, PREFIX.length).equals(Buffer.from(PREFIX))) continue;
    if (line.length > MAX_LINE) return refuse("phase-line-bound");
    if (record.records.length === MAX_RECORDS) return refuse("phase-record-bound");
    let row;
    try { row = JSON.parse(line.subarray(PREFIX.length).toString("utf8")); }
    catch { return refuse("phase-json"); }
    if (!row || Array.isArray(row) || Object.keys(row).length !== KEYS.length
        || !KEYS.every(key => Object.hasOwn(row, key))
        || row.schema !== "conduit.proof/host-startup@1" || !integer(pid) || pid < 1 || row.pid !== pid
        || row.sequence !== record.records.length || !integer(row.elapsed_us)
        || row.elapsed_us < previous || terminal) return refuse("phase-identity-or-order");
    const timing = row.duration_us === null || (integer(row.duration_us) && row.duration_us <= row.elapsed_us);
    if (!timing) return refuse("phase-duration");
    if (row.event === "start") {
      if (started || record.records.length || row.phase !== null || row.duration_us !== null) return refuse("phase-start");
      started = true; record.outcome = "incomplete";
    } else if (row.event === "begin") {
      const rank = PHASES.indexOf(row.phase);
      if (!started || failed || record.active_phase || rank <= lastPhase
          || PHASES.slice(0, rank).some(phase => !OPTIONAL.has(phase) && !seen.has(phase))
          || row.duration_us !== null) return refuse("phase-begin");
      lastPhase = rank;
      record.active_phase = row.phase; seen.add(row.phase);
    } else if (["end", "refused", "unwound"].includes(row.event)) {
      if (!started || !record.active_phase || row.phase !== record.active_phase
          || !integer(row.duration_us)) return refuse("phase-end");
      record.active_phase = null;
      if (row.event !== "end") { failed = true; record.failed_phase = row.phase; }
    } else if (["prepared", "aborted"].includes(row.event)) {
      if (!started || record.active_phase || row.phase !== null || row.duration_us !== null
          || (row.event === "prepared" && (failed
            || PHASES.some(phase => !OPTIONAL.has(phase) && !seen.has(phase))))) return refuse("phase-terminal");
      terminal = true; record.outcome = row.event;
    } else return refuse("phase-event");
    previous = row.elapsed_us; record.records.push(row);
  }
  if (record.output_truncated || record.incomplete_line) {
    record.outcome = "incomplete";
  }
  return record;
}
