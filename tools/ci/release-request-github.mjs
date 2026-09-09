#!/usr/bin/env node
import { execFileSync, spawnSync } from "node:child_process";
import { appendFileSync } from "node:fs";
import { requestRelease } from "./release-request.mjs";

const repository = process.env.GITHUB_REPOSITORY;
if (!/^[\w.-]+\/[\w.-]+$/.test(repository ?? "")) throw new Error("GITHUB_REPOSITORY must be owner/name");
const git = (...args) => execFileSync("git", args, { encoding: "utf8" }).trim();
const queryGit = (...args) => {
  const result = spawnSync("git", args, { encoding: "utf8" });
  if (result.error) throw result.error;
  if (![0, 1].includes(result.status)) throw new Error(result.stderr);
  return result;
};
const api = async (path, method = "GET", value) => {
  const args = ["api", `repos/${repository}${path}`, "--method", method];
  if (value) args.push("--input", "-");
  // Pull lists must not truncate away the release or synchronization owner.
  if (method === "GET" && path.startsWith("/pulls?")) args.push("--paginate", "--slurp");
  // A workflow run carries repository, actor, commit, URLs, and other nested
  // objects that release admission never reads. Hundreds of those complete
  // records can exceed Node's child-process buffer before admission gets to
  // classify them. Keep the response bounded to the decision's exact inputs.
  if (method === "GET" && path.startsWith("/actions/workflows/dev-integration.yml/runs?")) {
    args.push("--jq", "{workflow_runs: [.workflow_runs[] | {id, head_sha, head_branch, head_repository: {full_name: .head_repository.full_name}, event, status, conclusion, created_at}]}");
  }
  const output = execFileSync("gh", args, { encoding: "utf8", input: value ? JSON.stringify(value) : undefined });
  if (!output.trim()) return null;
  const result = JSON.parse(output);
  return args.includes("--slurp") ? result.flat() : result;
};
const result = await requestRelease({
  api, git, repository, integratedSha: process.env.INTEGRATION_SHA ?? "",
  isAncestor: (base, head) => queryGit("merge-base", "--is-ancestor", base, head).status === 0,
  mergedTree: (base, head) => {
    const result = queryGit("merge-tree", "--write-tree", base, head);
    return result.status === 0 ? result.stdout.trim() : null;
  },
});
process.stdout.write(`${JSON.stringify(result)}\n`);
if (process.env.GITHUB_STEP_SUMMARY) {
  appendFileSync(process.env.GITHUB_STEP_SUMMARY, `## Release admission\n\n\`\`\`json\n${JSON.stringify(result, null, 2)}\n\`\`\`\n`);
}
