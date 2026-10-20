#!/usr/bin/env bash
# Prepared only. Run after Root grants this cache and the sole Chromium slot.
set -euo pipefail
umask 077
# Internal producer handoff only: Root must grant the cache and sole browser slot.
if [ "${1:-}" = --execute-after-root-handoff ]; then shift; fi
if [ "$#" -ne 4 ]; then
  printf '%s\n' 'Usage: runner [--execute-after-root-handoff] EXPECTED_FULL_SOURCE_SHA LINUX_RELEASE_DIR SEALED_HANDBOOK_DIR FRESH_OUTPUT_DIR' >&2
  exit 2
fi
task_work=/home/dancxjo/Documents/Codex/2026-10-19/goal-6038/work
task_repo="$task_work/owner-presentation"
task_source=$1
[[ "$task_source" =~ ^[0-9a-f]{40}$ ]] || { printf '%s\n' 'Expected full 40-character Source SHA.' >&2; exit 2; }
task_release=$(realpath -e -- "$2")
task_handbook=$(realpath -e -- "$3")
task_output=$(realpath -m -- "$4")
cd "$task_repo"
test "$(git rev-parse HEAD)" = "$task_source"
test -z "$(git status --porcelain)"
test ! -e "$task_output"
test -d "$task_handbook"
test ! -L "$task_output"
test -f "$task_release/hosted-linux-x86_64.json"
test -x "$task_release/conduit-linux-x86_64"
mkdir "$task_output"
export CARGO_TARGET_DIR="$task_work/owner-installed-release-target"
export CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
df -h "$task_work" > "$task_output/disk-before.txt"
python3 - "$task_release/hosted-linux-x86_64.json" "$task_source" <<'PY'
import json,sys
assert json.load(open(sys.argv[1]))['source_identity']==sys.argv[2]
PY
# Already produced/sealed inputs only. Never build or package them here.
python3 - "$task_handbook/sdk/bundle/browser-bundle-release.json" "$task_source" <<'PYBUNDLE'
import json,sys
assert json.load(open(sys.argv[1]))['source_identity']==sys.argv[2]
PYBUNDLE
# State/checkpoint stay private and short. Do not archive authority credentials.
task_private=$(mktemp -d /tmp/c5645-current.XXXXXX)
task_state="$task_private/host"
task_checkpoint="$task_private/checkpoints"
mkdir "$task_state" "$task_checkpoint"
printf '%s\n' "$task_private" > "$task_output/private-state-path.txt"
task_installed=''
cleanup() {
  task_status=$?
  trap - EXIT
  # Normal cleanup is xtask's Service Drop (kill+wait). This catches only a
  # leaked service with this exact installed executable and private state.
  # Fallback is report-only: never signal a PID that could have been reused.
  python3 - "$task_installed" "$task_state" "$task_output/service-cleanup.json" <<'PY'
import pathlib,os,sys,json
binary,state,destination=sys.argv[1:];found=[]
if binary:
 for p in pathlib.Path('/proc').iterdir():
  if not p.name.isdecimal(): continue
  try:
   args=(p/'cmdline').read_bytes().split(b'\0')
   if p.stat().st_uid!=os.geteuid() or args[:4]!=[os.fsencode(binary),b'host',b'service',b'run']: continue
   index=args.index(b'--state-dir')
   if args[index+1]!=os.fsencode(state): continue
   start=(p/'stat').read_text().rsplit(')',1)[1].split()[19]
   found.append({'pid':int(p.name),'starttime_ticks':start,'disposition':'preserved; fallback cleanup is report-only'})
  except (OSError,ValueError,IndexError): continue
pathlib.Path(destination).write_text(json.dumps({'matching_owned_services_preserved':found,'normal_cleanup':'xtask Service Drop kills/waits; fallback sends no signals; private installation/checkpoints preserved'},indent=2)+'\n')
PY
  python3 - "$task_repo" "$task_source" "$task_status" "$task_output/final-source-state.json" <<'PYSTATE'
import json,pathlib,subprocess,sys
repo,expected,status,destination=sys.argv[1:]
head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip()
dirty=subprocess.check_output(['git','status','--porcelain'],cwd=repo,text=True)
pathlib.Path(destination).write_text(json.dumps({'expected_source':expected,'observed_head':head,'clean':not dirty,'producer_exit_code':int(status)},indent=2)+'\n')
PYSTATE
  exit "$task_status"
}
trap cleanup EXIT
"$task_release/conduit-linux-x86_64" host service install "$task_release/hosted-linux-x86_64.json" --state-dir "$task_state" --no-start --selected-todo-checkpoint-root "$task_checkpoint" > "$task_output/install.stdout" 2> "$task_output/install.stderr"
task_installed=$(python3 - "$task_state/installation.json" "$task_source" <<'PY'
import json,pathlib,sys
r=json.load(open(sys.argv[1]));assert r['release_source_identity']==sys.argv[2]
p=pathlib.Path(r['product_executable']);assert p.is_absolute() and p.is_file();print(p.resolve())
PY
)
test -x "$task_installed"
task_playwright=$(realpath proof/browser/node_modules/@playwright/test/index.mjs)
mkdir "$task_output/artifacts"
cp --reflink=auto -- "$task_installed" "$task_output/artifacts/installed-conduit"
cp --reflink=auto -- "$task_release/hosted-linux-x86_64.json" "$task_output/artifacts/hosted-linux-x86_64.json"
python3 - "$task_handbook" "$task_source" "$task_installed" "$task_output/input-pins.json" "$task_release/hosted-linux-x86_64.json" "$task_state/installation.json" <<'PY'
import pathlib,json,hashlib,sys
handbook=pathlib.Path(sys.argv[1]);source=sys.argv[2];binary=pathlib.Path(sys.argv[3])
bundle=json.loads((handbook/'sdk/bundle/browser-bundle-release.json').read_text());assert bundle['source_identity']==source
installation=json.loads(pathlib.Path(sys.argv[6]).read_text());assert installation['release_source_identity']==source
files=[binary,pathlib.Path(sys.argv[5]),handbook/'application.application.json',handbook/'sdk/bundle/browser-bundle-release.json']
pathlib.Path(sys.argv[4]).write_text(json.dumps({'source':source,'installed_release_source_identity':installation['release_source_identity'],'installed_product_executable':str(binary),'files':[{'path':str(p),'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in files]},indent=2)+'\n')
PY
# This supported producer owns its service, one browser/page and one attempt;
# it retains failure/terminal receipts itself. Output must not pre-exist.
cargo +stable xtask prove todo-journey --state-dir "$task_state" --conduit-bin "$task_installed" --fresh-body-source plots/todo/checkpoint-once.conduit --handbook-package "$task_handbook" --pinned-playwright "$task_playwright" --first-item-text 'Buy milk' --required-component --output "$task_output/journey" > "$task_output/todo-journey.log" 2>&1
python3 - "$task_output/journey" <<'PY'
import pathlib,json,sys
root=pathlib.Path(sys.argv[1]);live=json.loads((root/'live-run.json').read_text());assert live['error'] is None
browser=json.loads((root/'browser/receipt.json').read_text());assert browser['source_relation']=='exact-source'
required=json.loads((root/'browser/required-component/observations.json').read_text());assert required['result']=='passed'
PY
test "$(git rev-parse HEAD)" = "$task_source"
test -z "$(git status --porcelain)"
df -h "$task_work" > "$task_output/disk-after.txt"
printf '%s\n' "Installed-owner proof retained at $task_output"
