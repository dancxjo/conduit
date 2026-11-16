#!/usr/bin/env python3
"""Read-only ELF32 Thumb prologue/direct-BL evidence; not a worst-case bound.
Refuses an active firmware compiler/linker, snapshots one stable ELF, then runs
LLVM tools on that private temporary snapshot. No firmware or cache writes.
"""
import argparse
import collections
import hashlib
import json
from pathlib import Path
import re
import struct
import subprocess
import tempfile

DEFAULT = (
    '____embassy_main_task', 'poll_boundary::PollBoundary',
    'runtime::shared::run::{closure#0}', 'play_run::run::{closure#0}',
    'play::prepare', 'play::owners::prepare', 'NativeAuthority>::start',
    'PreparedServices<', 'play_service::presentation::prepare',
)

INSTRUCTION = re.compile(r'([\da-f]+):\s+(?:(?:[\da-f]{4}|[\da-f]{2})\s+)+\s*([^\d\s].*)')

def active_writers(crate):
    rows = subprocess.check_output(['ps', '-eo', 'pid=,comm=,args='], text=True)
    found = []
    for row in rows.splitlines():
        fields = row.split(None, 2)
        if len(fields) != 3:
            continue
        pid, comm, args = fields
        if (comm == 'rustc' and f'--crate-name {crate} ' in args) or comm in ('rust-lld', 'ld.lld'):
            found.append({'pid': int(pid), 'executable': comm})
    return found


def segments(data):
    if data[:6] != b'\x7fELF\x01\x01' or struct.unpack_from('<H', data, 18)[0] != 40:
        raise ValueError('requires little-endian ELF32 ARM')
    offset = struct.unpack_from('<I', data, 28)[0]
    size, count = struct.unpack_from('<HH', data, 42)
    return [struct.unpack_from('<IIIIIIII', data, offset + i * size) for i in range(count)]


def signed_word(data, loads, address):
    for kind, offset, virtual, _, size, _, _, _ in loads:
        if kind == 1 and virtual <= address and address + 4 <= virtual + size:
            return struct.unpack_from('<i', data, offset + address - virtual)[0]
    raise ValueError(f'unmapped literal {address:#x}')


def prologue(instructions, data, loads):
    frame, registers, proof = 0, {}, []
    for address, assembly in instructions[:16]:
        if re.match(r'(?:bl|bx|b|pop)\s', assembly):
            break
        match = re.search(r'push\s+\{([^}]+)\}', assembly)
        if match:
            count = 0
            for register in match[1].split(','):
                span = re.fullmatch(r'\s*r(\d+)-r(\d+)\s*', register)
                count += int(span[2]) - int(span[1]) + 1 if span else 1
            frame += 4 * count
            proof.append({'instruction': hex(address), 'push_bytes': 4 * count})
        match = re.search(r'sub\s+sp, #(0x[\da-f]+|\d+)', assembly)
        if match:
            count = int(match[1], 0)
            frame += count
            proof.append({'instruction': hex(address), 'subtract_bytes': count})
        match = re.search(r'ldr\s+(r\d+),.*@\s+(0x[\da-f]+)', assembly)
        if match:
            literal = int(match[2], 16)
            registers[match[1]] = (literal, signed_word(data, loads, literal))
        match = re.search(r'add\s+sp, (r\d+)', assembly)
        if match:
            if match[1] not in registers:
                raise ValueError(f'unknown SP register adjustment at {address:#x}')
            literal, adjustment = registers[match[1]]
            frame -= adjustment
            proof.append({'instruction': hex(address), 'literal': hex(literal), 'signed_adjustment': adjustment})
    return frame, proof


def measure(data, snapshot, tool_dir, patterns):
    loads = segments(data)
    symbols, all_symbols = {}, {}
    nm = subprocess.check_output([str(tool_dir / 'llvm-nm'), '-S', '--demangle', str(snapshot)], text=True)
    for line in nm.splitlines():
        match = re.match(r'([\da-f]+) ([\da-f]+) (\w) (.+)', line)
        if match:
            address, size = int(match[1], 16), int(match[2], 16)
            all_symbols[match[4]] = {'address': hex(address), 'bytes': size}
            if match[3].lower() == 't':
                symbols[address] = {'address': hex(address), 'bytes': size, 'name': match[4], 'instructions': []}
        else:
            match = re.match(r'([\da-f]+) (\w) (.+)', line)
            if match:
                all_symbols[match[3]] = {'address': hex(int(match[1], 16)), 'bytes': None}
    disassembly = subprocess.check_output([str(tool_dir / 'llvm-objdump'), '-d', str(snapshot)], text=True)
    current = None
    for line in disassembly.splitlines():
        header = re.match(r'([\da-f]+) <.+>:', line)
        if header:
            current = symbols.get(int(header[1], 16))
        match = INSTRUCTION.match(line)
        if current is not None and match:
            current['instructions'].append((int(match[1], 16), match[2]))
    for entry in symbols.values():
        entry['frame_bytes'], entry['prologue_evidence'] = prologue(entry['instructions'], data, loads)
        entry['direct_bl'] = []
        for address, assembly in entry.pop('instructions'):
            match = re.match(r'bl\s+(0x[\da-f]+)', assembly)
            if match and int(match[1], 16) in symbols:
                entry['direct_bl'].append({'callsite': hex(address), 'target': match[1]})
    selected = [s for s in symbols.values() if any(p in s['name'] for p in patterns)]
    roots = [a for a, s in symbols.items() if 'TaskStorage<' in s['name'] and '____embassy_main_task' in s['name']]
    paths = []
    if roots:
        root = roots[0]
        queue, seen = collections.deque([(root, [])]), {root}
        while queue:
            address, path = queue.popleft()
            entry = symbols[address]
            if entry in selected:
                chain = path + [{'function': hex(address), 'name': entry['name'], 'frame_bytes': entry['frame_bytes']}]
                paths.append({'frame_sum_bytes': sum(e['frame_bytes'] for e in chain), 'chain': chain})
            for edge in entry['direct_bl']:
                target = int(edge['target'], 16)
                if target not in seen:
                    seen.add(target)
                    queue.append((target, path + [{'function': hex(address), 'name': entry['name'], 'frame_bytes': entry['frame_bytes'], 'callsite': edge['callsite']}]))
    layout = {k: v for k, v in all_symbols.items() if k in ('__sheap', '__euninit', '__sdata', '_stack_start', '_stack_end') or ('POOL' in k and (v['bytes'] or 0) >= 8192)}
    return {'layout': layout, 'frames': selected, 'direct_call_paths': paths,
            'limitations': 'Local prologue allocation only; direct BL paths are exact callsite evidence, not a complete WCET/stack upper bound. Indirect calls, dynamic adjustments and interrupt nesting require separate accounting.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('elf', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--pattern', action='append', default=[])
    parser.add_argument('--tool-dir', type=Path, default=Path.home() / '.rustup/toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin')
    args = parser.parse_args()
    elf = args.elf.resolve()
    busy = active_writers(elf.name.replace('-', '_'))
    if busy:
        raise SystemExit('ELF inspection refused: active compiler/linker ' + json.dumps(busy))
    before = elf.stat()
    data = elf.read_bytes()
    with tempfile.TemporaryDirectory(prefix='conduit-stack-proof-') as directory:
        snapshot = Path(directory) / elf.name
        snapshot.write_bytes(data)
        result = measure(data, snapshot, args.tool_dir, tuple(args.pattern) or DEFAULT)
    after = elf.stat()
    if (before.st_ino, before.st_size, before.st_mtime_ns) != (after.st_ino, after.st_size, after.st_mtime_ns) or active_writers(elf.name.replace('-', '_')):
        raise SystemExit('ELF inspection refused: artifact or compiler changed during inspection')
    result.update({'elf': str(elf), 'sha256': hashlib.sha256(data).hexdigest(), 'elf_bytes': len(data)})
    encoded = json.dumps(result, indent=2) + '\n'
    if args.output:
        args.output.write_text(encoded)
    else:
        print(encoded, end='')

if __name__ == '__main__':
    main()
