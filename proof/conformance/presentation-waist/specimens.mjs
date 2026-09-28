import { readFileSync } from "node:fs";

/** Shared bounded fixtures consumed by both Node and Rust conformance. */
export const specimens = JSON.parse(
  readFileSync(new URL("./specimens.json", import.meta.url), "utf8"),
);
