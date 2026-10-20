#!/usr/bin/env python3
"""Private native runner: one Cargo compile; package-scoped exact artifact execution.

No tests run during Cargo compilation. Listing/targeted runs use artifact package
cwd and explicit Cargo package context. This is native-only (no target runner).
"""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import tomllib
import subprocess
import sys

PACKAGES = ['conduit-plot', 'conduit-language', 'conduit-ai',
            'conduit-std-host', 'conduit-speech']
FEATURES = ('conduit-language/parser-model,conduit-ai/kernel-operation-owners,'
            'conduit-ai/hosted-catalog-cache,conduit-std-host/analysis-round-model,'
            'conduit-speech/semantic-bindings,conduit-speech/kernel')
ANCHORS = [('conduit-plot', 'resource_reference_equality'),
           ('conduit-language', 'resource_lexicon_snapshot'),
           ('conduit-ai', 'fixed_numeric_dsp_conversion'),
           ('conduit-std-host', 'analysis_round_driver'),
           ('conduit-speech', 'fargan_carrier_epochs')]
parser = argparse.ArgumentParser()
parser.add_argument('--test', action='append', required=True,
                    help='target or package::target (required for ambiguous names)')
parser.add_argument('--filter')
parser.add_argument('--only-case', action='append')
parser.add_argument('--include-ignored', action='store_true')
parser.add_argument('--lint', action='store_true')
a = parser.parse_args()
if not sys.platform.startswith('linux'):
    parser.error('this private runner currently supports native Linux only')
if a.filter and a.only_case:
    parser.error('--filter and --only-case are mutually exclusive')
if os.environ.get('CARGO_BUILD_TARGET') or any(
        name.startswith('CARGO_TARGET_') and name.endswith('_RUNNER')
        for name in os.environ):
    parser.error('native direct execution does not support target/runner overrides')
evidence = Path(__file__).resolve().parent
root = (evidence.parent / 'carried-fore-dependency-check').resolve(strict=True)
target_dir = (root.parent / 'energy-target').resolve()
env = os.environ.copy()
env.update(RUSTUP_TOOLCHAIN='stable', CARGO_TARGET_DIR=str(target_dir),
           CARGO_BUILD_JOBS='4', CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0',
           CARGO_PROFILE_TEST_DEBUG='0', CARGO_PROFILE_TEST_OPT_LEVEL='1')
stamp = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S%fZ')
# Fail closed on configured build-target or runner semantics we do not reproduce.
configs = [parent / '.cargo' / name for parent in [root, *root.parents]
           for name in ('config', 'config.toml')]
cargo_home = Path(env.get('CARGO_HOME', str(Path.home() / '.cargo')))
configs += [cargo_home / name for name in ('config', 'config.toml')]
for config in dict.fromkeys(configs):
    if not config.is_file():
        continue
    settings = tomllib.loads(config.read_text())
    if settings.get('build', {}).get('target') or any(
            isinstance(v, dict) and 'runner' in v for v in settings.get('target', {}).values()):
        raise SystemExit('Unsupported build target/runner configuration: ' + str(config))


# Metadata reads package manifests; it does not compile or execute tests.
metadata = subprocess.run(['cargo', '+stable', 'metadata', '--locked', '--no-deps',
                           '--format-version', '1'], cwd=root, env=env,
                          text=True, capture_output=True)
if metadata.returncode:
    sys.stderr.write(metadata.stderr)
    sys.exit(metadata.returncode)
(evidence / ('coordinated-native-metadata-' + stamp + '.json')).write_text(metadata.stdout)
data = json.loads(metadata.stdout)
packages = {p['id']: p for p in data['packages'] if p['name'] in PACKAGES}
if {p['name'] for p in packages.values()} != set(PACKAGES):
    raise SystemExit('Exact five-package metadata inventory missing')
known = {}
for package in packages.values():
    for t in package['targets']:
        if t['kind'] == ['test']:
            known[(package['name'], t['name'])] = (package, t)
selected = []
for requested in a.test:
    matches = [key for key in known if requested == '::'.join(key)
               or ('::' not in requested and requested == key[1])]
    if len(matches) != 1:
        raise SystemExit('Missing/ambiguous target; use package::target: ' + requested)
    if matches[0] not in selected:
        selected.append(matches[0])
for key in selected + ANCHORS:
    package, _ = known.get(key, ({}, {}))
    if package:
        manifest_settings = tomllib.loads(Path(package['manifest_path']).read_text())
        for target in manifest_settings.get('test', []):
            if target.get('name') == key[1] and target.get('harness') is False:
                raise SystemExit('Non-libtest harness unsupported: ' + repr(key))
for anchor in ANCHORS:
    if anchor not in known:
        raise SystemExit('Missing exact compiled anchor: ' + repr(anchor))
compile_keys = list(dict.fromkeys(ANCHORS + selected))
cmd = ['cargo', '+stable', 'clippy' if a.lint else 'test', '--locked']
for name in PACKAGES:
    cmd += ['-p', name]
cmd += ['--features', FEATURES]
for target in dict.fromkeys(key[1] for key in compile_keys):
    cmd += ['--test', target]
if a.lint:
    sys.exit(subprocess.run(cmd + ['--', '-D', 'warnings'], cwd=root, env=env).returncode)
compile_log = evidence / ('coordinated-native-artifacts-' + stamp + '.jsonl')
print('Compiling exact graph once: ' + str(compile_log), flush=True)
with compile_log.open('x') as output:
    result = subprocess.run(cmd + ['--no-run', '--message-format=json-render-diagnostics'],
                            cwd=root, env=env, stdout=output, text=True)
if result.returncode:
    sys.exit(result.returncode)
artifacts = {}
binary_paths = {}
linked = []
for line in compile_log.read_text().splitlines():
    try:
        message = json.loads(line)
    except json.JSONDecodeError:
        continue  # Cargo non-JSON status, never interpreted as artifact authority.
    if message.get('reason') == 'build-script-executed':
        linked.extend(message.get('linked_paths', []))
    if message.get('reason') != 'compiler-artifact' or not message.get('executable'):
        continue
    package = packages.get(message['package_id'])
    target = message['target']
    if package is not None and target['kind'] == ['bin']:
        binary_paths[(package['id'], target['name'])] = str(Path(message['executable']).resolve(strict=True))
    if package is None or target['kind'] != ['test']:
        continue
    key = (package['name'], target['name'])
    if key not in compile_keys:
        continue
    if Path(target['src_path']).resolve() != Path(known[key][1]['src_path']).resolve():
        raise SystemExit('Artifact source disagrees with package metadata: ' + repr(key))
    executable = Path(message['executable']).resolve(strict=True)
    if not executable.is_relative_to(target_dir):
        raise SystemExit('Executable outside admitted target directory')
    if key in artifacts:
        raise SystemExit('Duplicate exact artifact: ' + repr(key))
    artifacts[key] = executable
if set(artifacts) != set(compile_keys):
    raise SystemExit('Missing exact artifacts: ' + repr(set(compile_keys) - set(artifacts)))

# Cargo normally extends the platform loader path for native test executables.
# Query the selected toolchain only; do not invoke another Cargo freshness pass.
libdir = subprocess.run(['rustc', '+stable', '--print', 'target-libdir'], cwd=root,
                        env=env, text=True, capture_output=True, check=True).stdout.strip()
loader = 'LD_LIBRARY_PATH'
cargo_binary = subprocess.run(['rustup', 'which', '--toolchain', 'stable', 'cargo'],
                              cwd=root, env=env, text=True, capture_output=True,
                              check=True).stdout.strip()

def context(key):
    package, _ = known[key]
    manifest = Path(package['manifest_path']).resolve(strict=True)
    runtime = env.copy()
    # Clear inherited package metadata rather than leak another package's context.
    for name in list(runtime):
        if name.startswith(('CARGO_PKG_', 'CARGO_BIN_EXE_')) or name in ('CARGO_MANIFEST_DIR', 'CARGO_MANIFEST_PATH'):
            del runtime[name]
    runtime.update(CARGO_MANIFEST_DIR=str(manifest.parent), CARGO_MANIFEST_PATH=str(manifest),
                   CARGO_TARGET_TMPDIR=str(target_dir / 'tmp'),
                   CARGO=cargo_binary, CARGO_CRATE_NAME=key[1].replace('-', '_'))
    runtime.pop('CARGO_BIN_NAME', None)
    for (package_id, name), path in binary_paths.items():
        if package_id == package['id']:
            runtime['CARGO_BIN_EXE_' + name] = path
    for suffix, value in [('VERSION', package['version']), ('NAME', package['name']),
                          ('AUTHORS', ':'.join(package['authors'])),
                          ('DESCRIPTION', package.get('description')),
                          ('HOMEPAGE', package.get('homepage')),
                          ('REPOSITORY', package.get('repository')),
                          ('LICENSE', package.get('license')),
                          ('LICENSE_FILE', package.get('license_file')),
                          ('README', package.get('readme')),
                          ('RUST_VERSION', package.get('rust_version'))]:
        runtime['CARGO_PKG_' + suffix] = value or ''
    numeric, _, prerelease = package['version'].split('+')[0].partition('-')
    for suffix, value in zip(['MAJOR', 'MINOR', 'PATCH'], numeric.split('.')):
        runtime['CARGO_PKG_VERSION_' + suffix] = value
    runtime['CARGO_PKG_VERSION_PRE'] = prerelease.split('+')[0]
    executable = artifacts[key]
    paths = []
    for item in linked:
        path = Path(item.split('=', 1)[-1]).resolve()
        # Cargo excludes native build paths outside its target directory.
        if path.is_relative_to(target_dir):
            paths.append(str(path))
    paths += [str(executable.parent.parent), str(executable.parent), libdir]
    paths += runtime.get(loader, '').split(os.pathsep)
    runtime[loader] = os.pathsep.join(dict.fromkeys(p for p in paths if p))
    return manifest.parent, runtime

inventories = {}
for key in compile_keys:
    cwd, runtime = context(key)
    listed = subprocess.run([str(artifacts[key]), '--list', '--format', 'terse'],
                            cwd=cwd, env=runtime, text=True, capture_output=True)
    listing = evidence / ('coordinated-native-list-' + stamp + '-' + '-'.join(key) + '.log')
    listing.write_text(listed.stdout + listed.stderr)
    if listed.returncode:
        raise SystemExit(listed.returncode)
    inventories[key] = [line[:-6] for line in listed.stdout.splitlines() if line.endswith(': test')]
available = {name for key in selected for name in inventories[key]}
if a.only_case and not set(a.only_case) <= available:
    raise SystemExit('Missing requested case: ' + repr(set(a.only_case) - available))
receipt = {'scope': 'native exact artifacts, package cwd; no installed/controller authority',
           'compile_log': str(compile_log), 'artifacts': []}
for key in selected:
    cwd, runtime = context(key)
    names = inventories[key]
    if a.filter and not any(a.filter in name for name in names):
        raise SystemExit('Filter matches no listed case in selected target: ' + repr(key))
    cases = [name for name in names if name in a.only_case] if a.only_case else [None]
    for case in cases:
        args = [str(artifacts[key]), '--nocapture', '--test-threads=1']
        if case is not None:
            args += ['--exact', case]
        elif a.filter:
            args.append(a.filter)
        if a.include_ignored:
            args.append('--include-ignored')
        binary_hash = hashlib.sha256(artifacts[key].read_bytes()).hexdigest()
        entry = {'package': key[0], 'package_id': known[key][0]['id'], 'target': key[1],
                 'executable': str(artifacts[key]), 'executable_sha256': binary_hash,
                 'cwd': str(cwd), 'arguments': args[1:], 'listed_cases': names,
                 'runtime_context': {k: v for k, v in runtime.items() if k.startswith(('CARGO_PKG_', 'CARGO_BIN_EXE_', 'CARGO_MANIFEST_')) or k in ('CARGO', 'CARGO_CRATE_NAME', 'CARGO_TARGET_TMPDIR', loader)}}
        receipt['artifacts'].append(entry)
        receipt_path = evidence / ('coordinated-native-execution-' + stamp + '.json')
        receipt_path.write_text(json.dumps(receipt, indent=2) + '\n')
        print('Running exact compiled proof: ' + repr(key) + ' cwd=' + str(cwd), flush=True)
        entry['returncode'] = subprocess.run(args, cwd=cwd, env=runtime).returncode
        entry['unchanged_binary'] = hashlib.sha256(artifacts[key].read_bytes()).hexdigest() == binary_hash
        receipt_path = evidence / ('coordinated-native-execution-' + stamp + '.json')
        receipt_path.write_text(json.dumps(receipt, indent=2) + '\n')
        if entry['returncode']:
            sys.exit(entry['returncode'])
        if not entry['unchanged_binary']:
            raise SystemExit('Artifact changed during execution; no exact-artifact conclusion')
sys.exit(0)
