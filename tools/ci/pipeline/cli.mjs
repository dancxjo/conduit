import { requireCaptureLineage } from './history-lineage.mjs';
import { checkEvidenceRetention } from './evidence-retention.mjs';
import { readDeliveryStatus } from './delivery-status.mjs';
import { appendFileSync, mkdirSync, readdirSync, existsSync, readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';
import { candidateBrowserProofRequired, planChanges, UNIT_SHARDS } from './plan.mjs';
import { admissionPlan, nativeRisk, runAdmission, isMeasurementReceipt } from './admission.mjs';
import { sealTarget, verifyBundle } from './receipts.mjs';
import { TARGETS, setupTarget, runTarget } from './targets.mjs';
import { setupCi, setupUnit } from './setup.mjs';
import { assertSourceCheckout } from './source.mjs';
import { emitAcquisitionKey, measureAcquisition } from './acquisition/metrics.mjs';
import { retainedOneBodyEvidence } from './one-body-evidence.mjs';
import { retainedThreeHostDevelopmentEvidence } from './three-host-development-evidence.mjs';
import { retainedDirectSpokenDevelopmentEvidence } from './direct-spoken-development-evidence.mjs';
import { retainedTodoBrowserDevelopmentEvidence } from './todo-browser-development-evidence.mjs';
import { beginProductCustody, finishProductCustody, abandonProductCustody } from './producer-custody.mjs';

function run(program, args, capture = false) {
  const result = spawnSync(program, args, { stdio: capture ? ['ignore', 'pipe', 'inherit'] : 'inherit', encoding: 'utf8' });
  if (result.error || result.status !== 0) throw new Error(`${program} ${args.join(' ')} failed: ${result.error ?? result.status}`);
  return result.stdout?.trim();
}
function exactSha(value) {
  if (!/^[a-f0-9]{40}$/.test(value ?? '')) throw new Error('expected full commit SHA');
  return value;
}
function output(key, value) {
  const line = `${key}=${typeof value === 'string' ? value : JSON.stringify(value)}\n`;
  process.stdout.write(line);
  if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, line);
}
function target(id) {
  const found = TARGETS.find(item => item.id === id);
  if (!found) throw new Error(`unknown target: ${id}`);
  return found;
}

const [command, ...args] = process.argv.slice(2);
try {
  switch (command) {
    case 'compiler-cache-hosted-restore': {
      if (args.length !== 3) throw new Error('usage: compiler-cache-hosted-restore <config> <new-acquisition-directory> <new-config>');
      const { loadHostedProductCacheConfig, writeHostedProductCacheConfig } = await import('./hosted-cache-command.mjs');
      const { hostedCachePolicy } = await import('../compiler-cache/cargo-sandbox.mjs');
      const { discoverAcceptedHostedSeeds, createHostedSeedObserver } = await import('./hosted-cache-discovery.mjs');
      const { acquireHostedCacheSeed } = await import('./hosted-cache-seed.mjs');
      const configuration = await loadHostedProductCacheConfig(resolve(args[0]));
      const { policy, namespace } = await hostedCachePolicy(configuration.snapshot, { maxCacheBytes: configuration.maxCacheBytes });
      const repository = process.env.GITHUB_REPOSITORY;
      const candidates = await discoverAcceptedHostedSeeds({ repository, namespace, firstOnly: true });
      let selectedConfig = resolve(args[0]);
      if (candidates.length) {
        const acquired = await acquireHostedCacheSeed({ ...candidates[0], policy, namespace, directory: resolve(args[1]) });
        createHostedSeedObserver({ repository }).accepted(candidates[0].accepted.source);
        selectedConfig = await writeHostedProductCacheConfig({ schema: 'conduit.hosted-cache-target/v1',
          snapshotManifest: configuration.snapshotManifest, cacheRoot: acquired.cacheRoot,
          taskRoot: configuration.taskRoot, cacheMode: configuration.cacheMode,
          maxCacheBytes: configuration.maxCacheBytes }, resolve(args[2]));
      }
      output('hosted-cache-config', selectedConfig);
      output('hosted-cache-restored', String(candidates.length !== 0));
      break;
    }
    case 'compiler-cache-hosted-seal': {
      if (args.length !== 2) throw new Error('usage: compiler-cache-hosted-seal <config> <new-seed-directory>');
      if (process.env.GITHUB_JOB !== 'target' || !/^[1-9][0-9]*$/.test(process.env.GITHUB_RUN_ID ?? '')) throw new Error('Compiler seed requires its actual target job');
      const { loadHostedProductCacheConfig } = await import('./hosted-cache-command.mjs');
      const { hostedCachePolicy } = await import('../compiler-cache/cargo-sandbox.mjs');
      const { createHostedSeedObserver } = await import('./hosted-cache-discovery.mjs');
      const { sealHostedCacheSeed } = await import('./hosted-cache-seed.mjs');
      const configuration = await loadHostedProductCacheConfig(resolve(args[0]), { completedTask: true });
      const { policy, namespace } = await hostedCachePolicy(configuration.snapshot, { maxCacheBytes: configuration.maxCacheBytes });
      const observed = createHostedSeedObserver({ repository: process.env.GITHUB_REPOSITORY }).api(`actions/runs/${process.env.GITHUB_RUN_ID}`);
      if (observed.run_attempt !== Number(process.env.GITHUB_RUN_ATTEMPT) || observed.head_sha !== process.env.GITHUB_SHA) throw new Error('Compiler seed run attempt differs from its job');
      const sealed = await sealHostedCacheSeed({ cacheRoot: configuration.cacheRoot, policy, namespace,
        source: configuration.snapshot.sourceCommit, tree: configuration.snapshot.sourceTree,
        run: observed, directory: resolve(args[1]) });
      output('hosted-seed-name', sealed.name);
      output('hosted-seed-archive', sealed.archive);
      break;
    }
    case 'compiler-cache-hosted-retire': {
      if (args.length < 4 || args.length > 103 || args.slice(3).some(value => !/^[1-9][0-9]*$/.test(value))) throw new Error('usage: compiler-cache-hosted-retire <repository> <namespace> <new-receipt-directory> <artifact-id>...');
      const { retireHostedCacheSeeds } = await import('./hosted-cache-seed.mjs');
      const retired = await retireHostedCacheSeeds({ repository: args[0], namespace: args[1],
        directory: resolve(args[2]), artifactIds: args.slice(3).map(Number) });
      output('hosted-seed-retired-ids', retired.artifacts.map(artifact => artifact.id));
      break;
    }
    case 'compiler-cache-hosted-prepare': {
      if (args.length !== 7) throw new Error('usage: compiler-cache-hosted-prepare <engine> <toolchain-root> <new-snapshot> <new-manifest> <cache-root> <new-task-root> <new-config>');
      const [engine, toolchainRoot, destination, manifestFile, cacheRoot, taskRoot, configFile] = args.map(value => resolve(value));
      const { prepareHostedProductCache } = await import('./hosted-cache-command.mjs');
      const prepared = await prepareHostedProductCache({ engine, toolchainRoot, destination, manifestFile, cacheRoot, taskRoot, configFile });
      output('hosted-cache-namespace', prepared.namespace);
      output('hosted-cache-config', prepared.configFile);
      output('hosted-cache-policy', prepared.managedPaths[0]);
      output('hosted-cache-objects', prepared.managedPaths[1]);
      break;
    }
    case 'history-lineage-create': {
      if (args.length !== 6) throw new Error('usage: history-lineage-create <original-repo> <mapped-repo> <commit-map> <capture-inventory> <immutable-custody> <new-policy-output>');
      const { prepareCaptureLineagePolicy } = await import('./history-lineage-create.mjs');
      const [originalDirectory, mappedDirectory, mapFile, capturesFile, custodyFile, outputFile] = args.map(value => resolve(value));
      output('history-lineage', await prepareCaptureLineagePolicy({ originalDirectory, mappedDirectory, mapFile, capturesFile, custodyFile, outputFile }));
      break;
    }
    case 'compiler-cache-proof': {
      if (args.length !== 1) throw new Error('usage: compiler-cache-proof <snapshot-manifest>');
      const specs = readdirSync('proof/ci').filter(name => name.startsWith('compiler-cache') && name.endsWith('.spec.mjs'));
      const result = spawnSync('node', ['--test', ...specs.map(name => `proof/ci/${name}`)], {
        stdio: 'inherit', env: { ...process.env,
          CONDUIT_CARGO_CACHE_SNAPSHOT: resolve(args[0]) },
      });
      if (result.error || result.status !== 0) throw new Error('Compiler cache proof failed');
      break;
    }
    case 'compiler-cache-engine': {
      if (args.length !== 1) throw new Error('usage: compiler-cache-engine <new-engine-directory>');
      const { acquirePinnedEngine } = await import('../compiler-cache/engine.mjs');
      output('compiler-cache-engine', await acquirePinnedEngine(resolve(args[0])));
      break;
    }
    case 'compiler-cache-prepare': {
      if (args.length !== 4) throw new Error('usage: compiler-cache-prepare <pinned-engine> <rust-toolchain-root> <new-snapshot> <new-manifest>');
      const { prepareCargoCache } = await import('../compiler-cache/command.mjs');
      const manifest = await prepareCargoCache(...args.map(value => resolve(value)));
      output('compiler-cache-source', manifest.sourceCommit);
      output('compiler-cache-snapshot', manifest.sha256);
      break;
    }
    case 'compiler-cache-build': {
      if (args.length !== 3) throw new Error('usage: compiler-cache-build <snapshot-manifest> <cache-directory> <new-task-directory>');
      const { buildCargoCache } = await import('../compiler-cache/command.mjs');
      const receipt = await buildCargoCache(...args.map(value => resolve(value)));
      output('compiler-cache-namespace', receipt.namespace);
      output('compiler-cache-validated-ms', String(receipt.validatedWallMs));
      break;
    }
    case 'compiler-cache-clear': {
      if (args.length !== 3) throw new Error('usage: compiler-cache-clear <cache-directory> <expected-namespace> <new-receipt>');
      const { clearCargoCache } = await import('../compiler-cache/command.mjs');
      const receipt = await clearCargoCache(resolve(args[0]), args[1], resolve(args[2]));
      output('compiler-cache-cleared-bytes', String(receipt.allocatedBytes));
      break;
    }
    case 'train-adoption': {
      if (args.length !== 1) throw new Error('usage: train-adoption <dev-integration-run-id>');
      if (!/^[1-9][0-9]*$/.test(args[0]) || !/^[\w.-]+\/[\w.-]+$/.test(process.env.GITHUB_REPOSITORY ?? '')) throw new Error('Invalid Actions run/repository');
      const repository = process.env.GITHUB_REPOSITORY;
      const pushRun = JSON.parse(run('gh', ['api', `repos/${repository}/actions/runs/${args[0]}`], true));
      assertSourceCheckout(pushRun.head_sha);
      const { discoverTrainAdoption } = await import('./train-adoption.mjs');
      const adoption = await discoverTrainAdoption({ pushRun, repository });
      output('adopted', String(adoption !== null));
      output('proof-run', String(adoption?.integrationRun ?? pushRun.id));
      break;
    }
    case 'train-service': {
      if ((args.length !== 4 && args.length !== 5) || (args.length === 5 && args[4] !== '--drain')) {
        throw new Error('usage: train-service <service-configuration> <external-machine-policy> <private-key-file> <trusted-policy-observer-environment-reference> [--drain]');
      }
      const { readTrainServiceConfiguration, runTrainServiceTick } = await import('./train-service.mjs');
      const { selectTrainServiceReadyInputs } = await import('./train-service-session.mjs');
      const { readMachineWriterConfiguration } = await import('./train-machine-policy.mjs');
      const configurationFile = resolve(args[0]);
      const service = await readTrainServiceConfiguration(configurationFile);
      const config = await readMachineWriterConfiguration(resolve(args[1]), service.sourceObjectStore);
      output('train-service-result', await runTrainServiceTick({ configurationFile, config,
        privateKeyFile: resolve(args[2]), observerEnvironmentReference: args[3], maintenance: args.length === 5,
        selectReady: selectTrainServiceReadyInputs }));
      break;
    }
    case 'train-supervise': {
      if ((args.length !== 5 && args.length !== 6) || (args.length === 6 && args[5] !== '--drain')) {
        throw new Error('usage: train-supervise <queue-file> <state-directory> <external-machine-policy> <private-key-file> <trusted-policy-observer-environment-reference> [--drain]');
      }
      const { readTrainSupervisorQueue, runTrainSupervisorTick } = await import('./train-supervisor.mjs');
      const { readMachineWriterConfiguration } = await import('./train-machine-policy.mjs');
      const queueFile = resolve(args[0]);
      const queue = await readTrainSupervisorQueue(queueFile);
      const config = await readMachineWriterConfiguration(resolve(args[2]), queue.targetSource);
      output('train-supervisor-result', await runTrainSupervisorTick({ queueFile,
        stateDirectory: resolve(args[1]), config, privateKeyFile: resolve(args[3]),
        observerEnvironmentReference: args[4], maintenance: args.length === 6 }));
      break;
    }
    case 'train-control': {
      if (args.length !== 4) throw new Error('usage: train-control <request-file> <external-machine-policy> <private-key-file> <trusted-policy-observer-environment-reference>');
      const { readTrainControllerRequest, runTrainController } = await import('./train-controller.mjs');
      const { readMachineWriterConfiguration } = await import('./train-machine-policy.mjs');
      const request = await readTrainControllerRequest(resolve(args[0]));
      const config = await readMachineWriterConfiguration(resolve(args[1]), request.targetSource);
      output('train-controller-result', await runTrainController({ request, config,
        privateKeyFile: resolve(args[2]), observerEnvironmentReference: args[3] }));
      break;
    }
    case 'train-admit': {
      if (args.length !== 1) throw new Error('usage: train-admit <manifest-file>');
      const { createTrainProofAdmission } = await import('./train-proof-admission.mjs');
      const proof = await createTrainProofAdmission()(resolve(args[0]));
      output('train-manifest-digest', proof.digest);
      break;
    }
    case 'train-land': {
      if (args.length !== 8) throw new Error('usage: train-land <manifest-file> <integration-run-id> <archive-directory> <new-evidence-directory> <external-machine-policy> <app-jwt-environment-reference> <trusted-policy-observer-environment-reference> <target-source-directory>');
      const { createTrainLander } = await import('./train-land.mjs');
      const { readMachineWriterConfiguration, createTrainCredentialApis } = await import('./train-machine-policy.mjs');
      const { createTrainCommands } = await import('./train-command.mjs');
      const { observeCheapTrainPolicy } = await import('./train-cheap-policy.mjs');
      const { createTrainSessionCommand, createTrainSourceEnvironment, createTrainMetadataApi } = await import('./train-controller.mjs');
      const directory = resolve(args[7]);
      const machineWriter = await readMachineWriterConfiguration(resolve(args[4]), directory);
      const { appIdentityApi, policyObserverApi } = createTrainCredentialApis({ machineWriter,
        appJwtEnvironmentReference: args[5], policyObserverEnvironmentReference: args[6] });
      const credentialReferences = [args[5], args[6]];
      const sourceEnvironment = createTrainSourceEnvironment(process.env, credentialReferences);
      const command = createTrainSessionCommand({ installationToken: process.env.GH_TOKEN, credentialReferences });
      const api = createTrainMetadataApi(command, directory);
      const commands = createTrainCommands({ directory, command, sourceEnvironment,
        observeCheapPolicy: ({ repository }) => observeCheapTrainPolicy({ config: machineWriter,
          repository, api, appApi: appIdentityApi, policyApi: policyObserverApi }) });
      const receipt = await createTrainLander({ directory, command, commands, machineWriter, appIdentityApi, policyObserverApi })(resolve(args[0]), args[1], resolve(args[2]), resolve(args[3]));
      output('train-landed-source', receipt.landedDev);
      output('train-landed-tree', receipt.tree);
      break;
    }
    case 'train-restore': {
      if (args.length !== 4) throw new Error('usage: train-restore <manifest-file> <integration-run-id> <archive-directory> <new-product-directory>');
      const { createTrainCommands } = await import('./train-command.mjs');
      const restored = await createTrainCommands().restore(resolve(args[0]), args[1], resolve(args[2]), resolve(args[3]));
      output('train-bundle', restored.bundle);
      output('train-integration-run', String(restored.runId));
      output('train-run-attempt', String(restored.runAttempt));
      break;
    }
    case 'train-acquire': {
      if (args.length !== 3) throw new Error('usage: train-acquire <manifest-file> <integration-run-id> <new-archive-directory>');
      const { createTrainCommands } = await import('./train-command.mjs');
      const receipt = await createTrainCommands().acquire(resolve(args[0]), args[1], resolve(args[2]));
      output('train-artifact-acquisition', receipt);
      break;
    }
    case 'train-dispatch': {
      if (args.length !== 1) throw new Error('usage: train-dispatch <manifest-file>');
      const { createTrainDispatcher } = await import('./train-dispatch.mjs');
      const result = await createTrainDispatcher()(resolve(args[0]));
      output('train-dispatch', result);
      break;
    }
    case 'train-assemble': {
      if (args.length < 4 || args.length > 35) throw new Error('usage: train-assemble <owner/repository> <new-worktree> <new-manifest-file> <pr-number>...');
      const { createTrainCommands } = await import('./train-command.mjs');
      const [repository, worktree, outputFile, ...numbers] = args;
      const manifest = await createTrainCommands().assemble(repository, numbers, resolve(worktree), resolve(outputFile));
      output('train-branch', manifest.branch);
      output('train-source', manifest.source);
      output('train-manifest-digest', manifest.digest);
      break;
    }
    case 'train-capture': {
      if (args.length < 4 || args.length > 35) throw new Error('usage: train-capture <owner/repository> <source-sha> <new-manifest-file> <pr-number>...');
      const { createTrainCommands } = await import('./train-command.mjs');
      const [repository, source, outputFile, ...numbers] = args;
      const manifest = await createTrainCommands().capture(repository, exactSha(source), numbers, resolve(outputFile));
      output('train-branch', manifest.branch);
      output('train-source', manifest.source);
      output('train-manifest-digest', manifest.digest);
      break;
    }
    case 'train-verify': {
      if (args.length !== 3) throw new Error('usage: train-verify <manifest-file> <integration-run-id> <bundle-directory>');
      const { createTrainCommands } = await import('./train-command.mjs');
      const verified = await createTrainCommands().verify(resolve(args[0]), args[1], resolve(args[2]));
      output('train-source', verified.source);
      output('train-tree', verified.tree);
      output('train-integration-run', String(verified.integrationRun));
      break;
    }
    case 'status':
      if (args.length !== 1) throw new Error('usage: status <owner/repository>');
      console.log(JSON.stringify(await readDeliveryStatus(args[0]), null, 2));
      break;
    case 'setup-ci':
      if (args.length) throw new Error('setup-ci accepts no arguments');
      await measureAcquisition('preflight', setupCi);
      break;
    case 'setup-unit':
      if (args.length) throw new Error('setup-unit accepts no arguments');
      await measureAcquisition('unit', setupUnit);
      break;
    case 'acquisition-key':
      if (args.length !== 1) throw new Error('usage: acquisition-key <target|preflight|unit>');
      if (!['preflight', 'unit'].includes(args[0])) target(args[0]);
      emitAcquisitionKey(args[0]);
      break;
    case 'scan': {
      if (args.length < 2 || args.length > 3 || (args[2] && args[2] !== 'candidate')) throw new Error('usage: scan <base-sha|all> <head-sha> [candidate]');
      const [base, head] = args;
      exactSha(head);
      const paths = base === 'all' ? [] : run('git', ['diff', '--name-only', '--no-renames', '-z', `${exactSha(base)}...${head}`], true).split('\0').filter(Boolean);
      const plan = planChanges(paths, { full: base === 'all' });
      if (args[2] === 'candidate') {
        plan.families = plan.families.filter(family =>
          (family === 'conduitos' && nativeRisk(paths))
          || (family === 'browser' && candidateBrowserProofRequired(paths))
          || paths.some(path => path.startsWith(`targets/${family}/firmware/`)));
      }
      let broadAdmission = false;
      if (args[2] === 'candidate' && !plan.docsOnly) {
        const metadata = JSON.parse(run('cargo', ['metadata', '--locked', '--format-version', '1', '--no-deps'], true));
        broadAdmission = admissionPlan(paths, metadata, process.cwd()).broad;
        // Broad admission previously tested every workspace package in one job.
        // Preserve that corpus even when target scanning selected fewer shards.
        if (broadAdmission) {
          plan.unitShards = [...UNIT_SHARDS];
          plan.conduitosProof = true;
        }
      }
      output('broad-admission', String(broadAdmission));
      output('has-targets', String(plan.families.length > 0));
      output('docs-only', String(plan.docsOnly));
      output('unit-matrix', { shard: plan.unitShards });
      output('conduitos-proof', String(plan.conduitosProof));
      output('matrix', { include: TARGETS.filter(item => plan.families.includes(item.family)) });
      output('sha', head);
      break;
    }
    case 'evidence-retention':
      if (args.length) throw new Error('evidence-retention accepts no arguments');
      checkEvidenceRetention();
      break;
    case 'preflight': {
      if (args.length !== 1) throw new Error('usage: preflight <base-sha>');
      checkEvidenceRetention();
      const oneBodyEvidence = retainedOneBodyEvidence();
      if (oneBodyEvidence) {
        // The browser target has a shallow checkout. Verify ancestry here,
        // where CI retains the full history, before any expensive target work.
        requireCaptureLineage(oneBodyEvidence.sourceCommit, run('git', ['rev-parse', 'HEAD'], true));
      }
      const threeHostDevelopment = oneBodyEvidence ? null : retainedThreeHostDevelopmentEvidence();
      if (threeHostDevelopment) {
        requireCaptureLineage(threeHostDevelopment.sourceCommit, run('git', ['rev-parse', 'HEAD'], true));
      }
      const directSpokenDevelopment = retainedDirectSpokenDevelopmentEvidence();
      if (directSpokenDevelopment) {
        requireCaptureLineage(directSpokenDevelopment.sourceCommit, run('git', ['rev-parse', 'HEAD'], true));
      }
      retainedTodoBrowserDevelopmentEvidence(undefined, run('git', ['rev-parse', 'HEAD'], true));
      run('git', ['diff', '--check', exactSha(args[0]), 'HEAD']);
      const changed = run('git', ['diff', '--name-only', '--no-renames', '-z', exactSha(args[0]), 'HEAD'], true).split('\0').filter(Boolean);
      for (const name of changed.filter(isMeasurementReceipt)) {
        if (existsSync(name)) {
          try { JSON.parse(readFileSync(name, 'utf8')); }
          catch (error) { throw new Error(`Invalid measurement receipt ${name}: ${error.message}`); }
        }
      }
      run('cargo', ['fmt', '--all', '--check']);
      run('cargo', ['metadata', '--locked', '--no-deps', '--format-version', '1'], true);
      // Physical definitions are authored in Conduitese; derived bindings must
      // remain reproducible from that source on every candidate.
      run('python3', ['architecture/core/definitions/generate.py', '--check']);
      run('cargo', ['xtask', 'ci', 'standalone-locks', '--locked']);
      // Keep the canonical corpus independent of edits to its Cargo wrapper.
      // Compile/run the actual std-only Rust launcher with real success/failure
      // probes, without replaying the full corpus or building xtask's graph.
      const specs = readdirSync('proof/ci').filter(name =>
        (name.startsWith('pipeline-') || name.startsWith('compiler-cache')) && name.endsWith('.spec.mjs'))
        .map(name => `proof/ci/${name}`);
      run('node', ['--test', ...specs]);
      const { runWorkflowContract } = await import('./workflow-contract.mjs');
      runWorkflowContract();
      // Syntax validation is required in CI, not a silently optional dependency.
      run(resolve('target/ci-tools/actionlint'), []);
      break;
    }
    case 'admission': {
      if (args.length < 1 || args.length > 2 || (args[1] && args[1] !== '--workspace-checked')) throw new Error('usage: admission <base-sha> [--workspace-checked]');
      const paths = run('git', ['diff', '--name-only', '--no-renames', '-z', `${exactSha(args[0])}...HEAD`], true).split('\0').filter(Boolean);
      const metadata = JSON.parse(run('cargo', ['metadata', '--locked', '--no-deps', '--format-version', '1'], true));
      runAdmission(paths, metadata, process.cwd(), run, { workspaceChecked: args[1] === '--workspace-checked' });
      break;
    }
    case 'setup':
      if (args.length !== 1) throw new Error('usage: setup <target>');
      target(args[0]);
      await measureAcquisition(args[0], () => setupTarget(args[0]));
      break;
    case 'target': {
      if (args.length !== 2) throw new Error('usage: target <target> <sha>');
      const item = target(args[0]);
      const sha = exactSha(args[1]);
      assertSourceCheckout(sha);
      const directory = resolve('target/pipeline', item.id);
      if (existsSync(directory)) throw new Error(`refusing stale target output: ${directory}`);
      mkdirSync(directory, { recursive: true });
      beginProductCustody({ source: sha, target: item.id });
      await runTarget(item.id, directory);
      assertSourceCheckout(sha);
      finishProductCustody(directory);
      await sealTarget({ directory, target: item.id, sha, proofClass: item.proofClass });
      console.log(`PASS: ${item.id} (${item.proofClass}) at ${sha}`);
      break;
    }
    case 'verify':
      if (args.length !== 2) throw new Error('usage: verify <bundle-directory> <sha>');
      await verifyBundle({ directory: resolve(args[0]), sha: exactSha(args[1]), targets: TARGETS });
      console.log('All required target artifacts and source identities verified.');
      break;
    case 'publish': {
      const { publish } = await import('./publish.mjs');
      if (args.length < 1 || args.length > 2) throw new Error('usage: publish <integration-run-id> [legacy-bundle-directory]');
      await publish(args[0], resolve(args[1] ?? 'target/bundle'));
      break;
    }
    default: throw new Error('usage: cargo xtask ci pipeline <scan|preflight|admission|unit|setup|target|verify|publish|train-service|train-supervise|train-control|train-assemble|train-capture|train-dispatch|train-acquire|train-restore|train-land|train-verify>');
  }
} catch (error) {
  abandonProductCustody();
  if (typeof error.result?.stderr === 'string' && error.result.stderr) {
    console.error(error.result.stderr.slice(-8192));
  }
  console.error(`FAILED: pipeline/${command}: ${error.message}`);
  process.exitCode = 1;
}
