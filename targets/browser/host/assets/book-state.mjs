const encoder = new TextEncoder();
const MAXIMUM_DRAFTS = 32;
const MAXIMUM_EXPANDED_BACKS = 16;
const MAXIMUM_STATE_KEY_BYTES = 128;
const MAXIMUM_DRAFT_BYTES = 4096;

function validKey(value) {
  return typeof value === "string" && value.length > 0 && encoder.encode(value).length <= MAXIMUM_STATE_KEY_BYTES;
}

export async function openBookReadingState(storage) {
  const drafts = new Map();
  const expandedBacks = new Set();
  let pending = Promise.resolve();
  const state = await storage.readJson("reading-state");
  if (state !== null) {
    if (state?.schema !== "conduit.book/reading-state@1"
      || !Array.isArray(state.drafts) || state.drafts.length > MAXIMUM_DRAFTS
      || !Array.isArray(state.expandedBacks) || state.expandedBacks.length > MAXIMUM_EXPANDED_BACKS) {
      throw new Error("persisted Book state is malformed");
    }
    for (const entry of state.drafts) {
      if (!Array.isArray(entry) || entry.length !== 2 || !validKey(entry[0])
        || typeof entry[1] !== "string" || encoder.encode(entry[1]).length > MAXIMUM_DRAFT_BYTES) {
        throw new Error("persisted Book draft is outside its admitted bound");
      }
      drafts.set(entry[0], entry[1]);
    }
    for (const key of state.expandedBacks) {
      if (!validKey(key)) throw new Error("persisted Book Back state is outside its admitted bound");
      expandedBacks.add(key);
    }
  }

  function persist() {
    const document = {
      schema: "conduit.book/reading-state@1",
      drafts: [...drafts.entries()].slice(0, MAXIMUM_DRAFTS),
      expandedBacks: [...expandedBacks].slice(0, MAXIMUM_EXPANDED_BACKS),
    };
    pending = pending.then(() => storage.writeJson("reading-state", document));
    return pending;
  }

  return Object.freeze({
    schema: "conduit.book/reading-state-handle@1",
    drafts,
    expandedBacks,
    persist,
    flush: () => pending,
  });
}
