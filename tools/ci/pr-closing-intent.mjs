#!/usr/bin/env node

import fs from "node:fs";
import { pathToFileURL } from "node:url";

const NEGATED_CLOSING_REFERENCE =
  /\b(?:(?:do|does|did|will|would|should|must|is|are)\s+not|(?:does|did|is|are|would|should|will|must)n['’]t)\s+(?:close[sd]?|fix(?:e[sd])?|resolve[sd]?)\s+(?:issue\s+)?#\d+\b/giu;

export function negatedClosingReferences(body) {
  return [...String(body ?? "").matchAll(NEGATED_CLOSING_REFERENCE)].map((match) => match[0]);
}

export function verifyPullRequestClosingIntent(event) {
  const body = event?.pull_request?.body ?? "";
  const references = negatedClosingReferences(body);
  if (references.length === 0) return;

  throw new Error(
    `PR prose contains a negated GitHub closing keyword (${references.join(", ")}). ` +
      "GitHub still treats that wording as a closure directive. Use 'Advances #123' or " +
      "'leaves #123 open' for partial work.",
  );
}

function main() {
  const eventPath = process.env.GITHUB_EVENT_PATH;
  if (!eventPath) throw new Error("GITHUB_EVENT_PATH is required");
  verifyPullRequestClosingIntent(JSON.parse(fs.readFileSync(eventPath, "utf8")));
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
