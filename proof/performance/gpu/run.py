"""Finite local CUDA feasibility, invoked by cargo xtask; never admits a Back."""
import argparse
import ctypes
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import sys
import time

BLOB_SHA = 'd35f0510da476716183a33caafe45cc095e48d496f06de2db7655e780a385e47'
UPSTREAM = '503d81b138d76621aae4b12786e90de48aa8db3a'
SOURCE_SHA = '68ba6e8731007f7184be862d7d1afe4cef096a7c9bcbfa115962340f725e004c'
CASES = [('sig_net_gru1_input_weights_float', 272, 480,
          'c9297da108f5cd11e76b714cd733e108f0181e9a7aba3737e377fc9b31380784'),
         ('sig_net_sig_dense_out_weights_float', 128, 40,
          'ef04dadd3930eeea7dcb94e66e9d34514369b14d016da14aa0a03abc797ef182')]
ROOT = Path(__file__).resolve().parents[3]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def git(*args):
    return subprocess.check_output(['git', '-C', str(ROOT), *args], text=True).strip()


def device(ordinal):
    lib = ctypes.CDLL('libcuda.so.1')
    def call(name, *args):
        status = getattr(lib, name)(*args)
        if status:
            raise RuntimeError(f'{name}:CUDA-driver-status-{status}')
    call('cuInit', 0)
    count = ctypes.c_int()
    call('cuDeviceGetCount', ctypes.byref(count))
    if not 0 <= ordinal < count.value:
        raise RuntimeError('selected-device-unavailable')
    dev = ctypes.c_int()
    call('cuDeviceGet', ctypes.byref(dev), ordinal)
    name = ctypes.create_string_buffer(256)
    call('cuDeviceGetName', name, len(name), dev)
    class UUID(ctypes.Structure):
        _fields_ = [('value', ctypes.c_ubyte * 16)]
    uuid = UUID()
    major, minor, driver = ctypes.c_int(), ctypes.c_int(), ctypes.c_int()
    call('cuDeviceGetUuid', ctypes.byref(uuid), dev)
    call('cuDeviceComputeCapability', ctypes.byref(major), ctypes.byref(minor), dev)
    call('cuDriverGetVersion', ctypes.byref(driver))
    return dict(ordinal=ordinal, name=name.value.decode(), uuid_hex=bytes(uuid.value).hex(),
                compute=[major.value, minor.value], driver_api=driver.value)


def fixture(root):
    started = time.perf_counter_ns()
    manifest = json.loads((root / 'resources/manifest.json').read_text())
    if manifest['upstream_sha'] != UPSTREAM or manifest['source_sha256'] != SOURCE_SHA:
        raise RuntimeError('pinned-source-refusal')
    blob_path = root / 'resources/f32.bin'
    blob = blob_path.read_bytes()
    if len(blob) != 3272868 or hashlib.sha256(blob).hexdigest() != BLOB_SHA:
        raise RuntimeError('pinned-model-content-refusal')
    arrays = {a['name']: a for a in manifest['arrays']}
    selected = []
    for name, inputs, outputs, expected_sha in CASES:
        array = arrays[name]
        span = array['profiles']['f32']
        offset, size = span['offset'], span['bytes']
        if (array['sha256'] != expected_sha or array['type'] != 'f32le' or offset < 0 or size != inputs * outputs * 4
                or offset + size > len(blob)
                or hashlib.sha256(blob[offset:offset + size]).hexdigest() != array['sha256']):
            raise RuntimeError('exact-tensor-span-refusal:' + name)
        selected.append(dict(name=name, inputs=inputs, outputs=outputs,
                             offset=offset, bytes=size, sha256=array['sha256']))
    return blob_path, selected, time.perf_counter_ns() - started


def process(args, prefix, env=None):
    started = time.perf_counter_ns()
    stdout_path, stderr_path = prefix.with_suffix('.jsonl'), prefix.with_suffix('.stderr')
    timeout = False
    with stdout_path.open('w') as stdout, stderr_path.open('w') as stderr:
        try:
            result = subprocess.run(args, stdout=stdout, stderr=stderr, text=True, env=env, timeout=120)
            code = result.returncode
        except subprocess.TimeoutExpired:
            timeout, code = True, 124
    elapsed = time.perf_counter_ns() - started
    inspection = time.perf_counter_ns()
    rows = [json.loads(line) for line in stdout_path.read_text().splitlines()]
    return dict(exit_code=code, timed_out=timeout, process_wall_ns=elapsed, rows=rows,
                raw_inspection_ns=time.perf_counter_ns() - inspection)


def run(args, out):
    if git('status', '--porcelain'):
        raise RuntimeError('freeze-and-commit-source-before-measurement')
    custody = dict(recorded_at_utc=datetime.now(timezone.utc).isoformat(), source=git('rev-parse', 'HEAD'), tree=git('rev-parse', 'HEAD^{tree}'),
                   accepted_main=git('rev-parse', 'refs/remotes/origin/main'),
                   host=platform.node(), platform=platform.platform(),
                   blob_sha256=BLOB_SHA, upstream_source=UPSTREAM)
    save(out / 'source.json', custody)
    blob, selected, validation_ns = fixture(args.fargan_fixture.resolve())
    save(out / 'resources.json', dict(arrays=selected, validation_ns=validation_ns))
    gpu = device(args.device)
    save(out / 'device.json', gpu)
    compiler = shutil.which('nvcc')
    host_compiler = shutil.which(args.host_compiler)
    if not compiler or not host_compiler:
        raise RuntimeError('explicit-CUDA-or-host-compiler-unavailable')
    worker = out / 'worker'
    flags = ['-ccbin', host_compiler, '-std=c++17', '-O3',
             f'-arch=sm_{gpu["compute"][0]}{gpu["compute"][1]}', '--fmad=false', '--ftz=false',
             '-Xcompiler=-ffp-contract=off,-fno-fast-math,-march=native']
    compile_args = [compiler, *flags, str(ROOT / 'proof/performance/gpu/fargan_projection.cu'),
                    '-o', str(worker)]
    started = time.perf_counter_ns()
    with (out / 'compiler.stdout').open('w') as stdout, (out / 'compiler.stderr').open('w') as stderr:
        compilation = subprocess.run(compile_args, stdout=stdout, stderr=stderr, text=True, timeout=120)
    compile_ns = time.perf_counter_ns() - started
    save(out / 'compiler.json', dict(flags=flags, compile_ns=compile_ns,
         cuda_version=subprocess.check_output([compiler, '--version'], text=True),
         host_version=subprocess.check_output([host_compiler, '--version'], text=True),
         exit_code=compilation.returncode))
    if compilation.returncode:
        raise RuntimeError('CUDA-compilation-refusal')
    custody['worker_sha256'] = digest(worker)
    save(out / 'source.json', custody)
    records, summary, valid = [], [], True
    for array in selected:
        for mode in ['normal', 'tiny-normal', 'subnormal']:
            argv = [str(worker), str(blob), str(array['offset']), str(array['inputs']),
                    str(array['outputs']), str(args.repetitions), str(args.device),
                    str(args.quota_mib * 1024 * 1024), mode]
            record = process(argv, out / f'{array["inputs"]}x{array["outputs"]}-{mode}')
            record.update(array=array['name'], mode=mode)
            records.append(record)
            trials = [r for r in record['rows'] if r['record'] == 'trial']
            success = (record['exit_code'] == 0 and len(trials) == args.repetitions
                       and all(t['equal_bits'] for t in trials)
                       and record['rows'][-1].get('success') is True)
            valid &= success
            summary.append(dict(array=array['name'], mode=mode, valid=success,
                cpu_median_ns=statistics.median(t['cpu_ns'] for t in trials) if trials else None,
                gpu_complete_median_ns=statistics.median(t['gpu_complete_ns'] for t in trials) if trials else None))
    array = selected[0]
    for label, ordinal, quota, hidden, reason in [
        ('quota-refusal', args.device, 0, False, 'finite-device-memory-quota-refusal'),
        ('missing-device', 2147483647, args.quota_mib, False, 'cudaErrorInvalidDevice'),
        ('no-visible-device', 0, args.quota_mib, True, 'cudaErrorNoDevice')]:
        env = dict(os.environ)
        if hidden:
            env['CUDA_VISIBLE_DEVICES'] = ''
        argv = [str(worker), str(blob), str(array['offset']), str(array['inputs']),
                str(array['outputs']), '3', str(ordinal), str(quota * 1024 * 1024), 'normal']
        record = process(argv, out / label, env)
        record['control'] = label
        records.append(record)
        valid &= (record['exit_code'] == 2 and len(record['rows']) == 1
                  and reason in record['rows'][0].get('reason', '')
                  and record['rows'][0].get('cleanup_success') is True)
    normal = [s for s in summary if s['mode'] == 'normal']
    decision = 'incomplete-or-refused'
    if valid:
        decision = ('defer-per-Gear-provider' if any(s['gpu_complete_median_ns'] >= s['cpu_median_ns']
                    for s in normal) else 'local-benefit-observed-provider-still-unproved')
    save(out / 'observation.json', dict(schema='conduit.gpu-projection-feasibility@2',
         custody=custody, device=gpu, resources=selected, summary=summary, records=records,
         decision=decision, conformance_passed=bool(valid), conduit_gpu_offer_admitted=False,
         limitations=['Actual pinned model weights with controlled finite feature inputs; ordered unbiased F32 projection only, not complete Conduit Plan/Body/dense/GRU/frame execution.',
           'Uncalibrated local family, fixed CPU-first order; every cold/first trial and refusal retained without retries.',
           'Quota covers three explicit device buffers, not opaque CUDA context/driver overhead. No Conduit resource or authority admission is claimed.',
           'Subnormal controls may favor GPU execution; this is not a beneficial ordinary FARGAN workload qualification.',
           'Process wall includes startup and raw output capture; worker owner includes file read, host buffers, context, transfers, execution and disposal. JSON inspection follows execution.']))
    if not valid:
        raise RuntimeError('GPU-family-conformance-refusal; raw records retained')
    print(out / 'observation.json')


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--fargan-fixture', type=Path, required=True)
    p.add_argument('--output', type=Path, default=ROOT / 'target/gpu-feasibility')
    p.add_argument('--repetitions', type=int, default=9)
    p.add_argument('--device', type=int, default=0)
    p.add_argument('--quota-mib', type=int, default=1)
    p.add_argument('--host-compiler', default='g++-13')
    args = p.parse_args()
    if not 3 <= args.repetitions <= 101 or not 1 <= args.quota_mib <= 16 or args.device < 0:
        p.error('finite bounds: repetitions 3..101, quota 1..16 MiB, device >= 0')
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    try:
        run(args, out)
    except (RuntimeError, OSError, ValueError, KeyError, TypeError, AttributeError, IndexError, subprocess.SubprocessError) as error:
        save(out / 'refusal.json', dict(reason=str(error), conduit_gpu_offer_admitted=False))
        print(error, file=sys.stderr)
        return 2
    return 0


if __name__ == '__main__':
    sys.exit(main())
