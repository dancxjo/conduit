#!/usr/bin/env python3
"""Private runner v2: compile/list once, run exact reported binaries; no second Cargo wait."""
import argparse
import datetime
import os
from pathlib import Path
import re
import subprocess
import sys

parser = argparse.ArgumentParser()
parser.add_argument('--test', action='append', required=True)
parser.add_argument('--filter')
parser.add_argument('--only-case', action='append', help='Run only these exact listed case names')
parser.add_argument('--include-ignored', action='store_true')
parser.add_argument('--lint', action='store_true', help='Clippy the same selected dev graph without running tests')
a = parser.parse_args()
evidence = Path(__file__).resolve().parent
root = evidence.parent / 'carried-fore-dependency-check'
anchors = ['resource_reference_equality', 'resource_lexicon_snapshot', 'fixed_numeric_dsp_conversion', 'analysis_round_driver', 'fargan_carrier_epochs']
targets = list(dict.fromkeys(anchors + a.test))
env = os.environ.copy()
env.update(RUSTUP_TOOLCHAIN='stable', CARGO_TARGET_DIR=str(root.parent / 'energy-target'),
           CARGO_BUILD_JOBS='4', CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0',
           CARGO_PROFILE_TEST_DEBUG='0', CARGO_PROFILE_TEST_OPT_LEVEL='1')
cmd = ['cargo', '+stable', 'test', '--locked', '-p', 'conduit-plot', '-p', 'conduit-language',
       '-p', 'conduit-ai', '-p', 'conduit-std-host', '-p', 'conduit-speech', '--features',
       'conduit-language/parser-model,conduit-ai/kernel-operation-owners,conduit-ai/hosted-catalog-cache,conduit-std-host/analysis-round-model,conduit-speech/semantic-bindings,conduit-speech/kernel']
for target in targets:
    cmd += ['--test', target]
if a.lint:
    cmd[2] = 'clippy'
    sys.exit(subprocess.run(cmd + ['--', '-D', 'warnings'], cwd=root, env=env).returncode)
# Listing compiles the exact same graph, without executing any proof or ignored fixture.
stamp = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S%fZ')
listing_path = evidence / ('coordinated-native-list-' + stamp + '.log')
print('Compiling/listing exact dev graph; live log: ' + str(listing_path), flush=True)
with listing_path.open('w') as output:
    listed = subprocess.run(cmd + ['--', '--list', '--format', 'terse'], cwd=root, env=env,
                            stdout=output, stderr=subprocess.STDOUT, text=True)
listing = listing_path.read_text()
if listed.returncode:
    print(listing, end='')
    sys.exit(listed.returncode)
by_target = {}
artifacts = []
current = None
for line in listing.splitlines():
    match = re.search(r'Running tests/(\w+)\.rs \(([^)]+)\)', line)
    if match:
        current = match.group(1)
        cases = []
        by_target.setdefault(current, []).extend(cases)
        artifacts.append((current, Path(match.group(2)), cases))
    elif current and line.endswith(': test'):
        artifacts[-1][2].append(line[:-6])
        by_target[current].append(line[:-6])
for anchor in anchors:
    if anchor not in by_target:
        raise SystemExit('Missing compiled anchor inventory: ' + anchor)
args = ['--', '--nocapture', '--test-threads=1']
if a.filter:
    args.append(a.filter)
if a.include_ignored:
    args.append('--include-ignored')
skipped = 0
for anchor in anchors:
    if anchor not in a.test:
        for name in by_target[anchor]:
            args += ['--skip', name]
            skipped += 1
if a.only_case:
    available = {name for names in by_target.values() for name in names}
    if not set(a.only_case) <= available:
        raise SystemExit('Missing requested cases: ' + repr(set(a.only_case) - available))
    for name in sorted(available - set(a.only_case)):
        args += ['--skip', name]
print('Stable Plot/Language/AI/Std/Speech dev graph; %d unrelated anchor cases skipped.' % skipped, flush=True)
# The first Cargo invocation has already compiled and listed these exact executables.
# Execute only requested targets; unrelated anchors remain compile-only cache anchors.
# This avoids a second Cargo freshness pass and build-lock wait during proof execution.
for name, executable, _ in artifacts:
    if name not in a.test:
        continue
    executable = executable.resolve(strict=True)
    assert executable.is_relative_to((root.parent / 'energy-target').resolve())
    print('Running exact compiled proof: ' + str(executable), flush=True)
    result = subprocess.run([str(executable)] + args[1:], cwd=root, env=env)
    if result.returncode:
        sys.exit(result.returncode)
sys.exit(0)
