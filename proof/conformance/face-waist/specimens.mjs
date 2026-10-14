import { readFileSync } from "node:fs";

/** Shared bounded fixtures consumed by both Node and Rust conformance. */
export const specimens = JSON.parse(
  readFileSync(new URL("./specimens.json", import.meta.url), "utf8"),
);

/** Independent human accounts for the bounded reconstructions, not Shows. */
export const accounts = JSON.parse(
  readFileSync(new URL("./accounts.json", import.meta.url), "utf8"),
);
