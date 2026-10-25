import { createHash } from 'node:crypto';

// Inert archive fixture, never evidence of actual target execution.
export function producerFixture({ source, tree, target, run }) {
  const report = { target, sourceSha: source, outcome: 'success', tools: [{ name: 'fixture', version: 'inert' }] };
  const digest = value => createHash('sha256').update(value).digest('hex');
  return {
    schema: 'conduit.pipeline-producer/v1', source, tree, target,
    run: { repository: run.repository.full_name, repositoryId: run.repository.id,
      id: run.id, attempt: run.run_attempt, workflowSource: source, source, job: 'target',
      workflow: `${run.repository.full_name}/.github/workflows/integration.yml@refs/heads/${run.head_branch}` },
    toolchain: { rustc: 'fixture', cargo: 'fixture', node: 'fixture',
      acquisition: { report, sha256: digest(JSON.stringify(report, null, 2) + '\n') } },
    environment: { sha256: digest('fixture'), platform: 'linux', arch: 'x64', kernel: 'fixture',
      image: { os: 'fixture', version: 'fixture' } },
    selection: { inventory: 'source-bound-command-contract', policyTree: tree,
      commands: [{ program: 'fixture', args: [], outcome: 'success', durationMs: 0,
        cwd: '/fixture', environmentSha256: digest('fixture') }] }, durationMs: 0,
  };
}
