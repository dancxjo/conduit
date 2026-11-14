#!/usr/bin/env python3
"""Hardware PROOF byte fixture, not a production Line or authority provider.

READY\n means only that this byte forwarder is available. USB serial numbers,
SSH carriage and TIOCEXCL do not authenticate the physical peer or confer grants.
Opening/closing a TTY can change modem lines; this program promises no reset-free
handoff. Do not replace a live bridge without separately authorized recovery.
"""
import argparse
import fcntl
import json
import os
from pathlib import Path
import select
import stat
import sys
import termios
import time

CHUNK = 4096


def emit(event, **fields):
    record = dict(classification="hardware-PROOF-fixture", event=event,
                  native_line_ready=False, execution_authority=False, **fields)
    data = (json.dumps(record, separators=(",", ":")) + "\n").encode()
    if len(data) > 8192:
        raise ValueError("native evidence exceeds bounded stderr record")
    sys.stderr.buffer.write(data)
    sys.stderr.buffer.flush()


def modem(fd):
    import array
    bits = array.array("i", [0])
    fcntl.ioctl(fd, termios.TIOCMGET, bits, True)
    return bits[0]


def usb_mapping(rdev):
    node = Path(f"/sys/dev/char/{os.major(rdev)}:{os.minor(rdev)}/device").resolve(strict=True)
    for parent in (node, *node.parents):
        if (parent / "idVendor").exists():
            result = {name: (parent / name).read_text().strip()
                      for name in ("idVendor", "idProduct", "serial", "product")
                      if (parent / name).exists()}
            result["sysfs"] = str(parent)
            if not result.get("serial") or any(len(v) > 512 for v in result.values()):
                raise ValueError("USB mapping absent or unbounded")
            return result
    raise ValueError("selected TTY has no independently inspectable USB mapping")


def census(rdev, own_fd):
    owners, unknown = set(), 0
    for index, process in enumerate(Path("/proc").glob("[0-9]*")):
        if index >= 16384:
            unknown += 1
            break
        try:
            for number, descriptor in enumerate((process / "fd").iterdir()):
                if number >= 65536:
                    unknown += 1
                    break
                try:
                    info = descriptor.stat()
                except PermissionError:
                    unknown += 1
                    continue
                except FileNotFoundError:
                    continue
                if stat.S_ISCHR(info.st_mode) and info.st_rdev == rdev:
                    pair = (int(process.name), int(descriptor.name))
                    if pair != (os.getpid(), own_fd):
                        owners.add(pair)
                        if len(owners) > 16:
                            raise ValueError("competing opener census exceeds proof bound")
        except PermissionError:
            unknown += 1
        except FileNotFoundError:
            continue
    return sorted(owners), unknown


def snapshot(fd):
    attrs = termios.tcgetattr(fd)
    return dict(modem=modem(fd), termios=attrs[:6], cc=[v if isinstance(v, int) else v[0] for v in attrs[6]], hupcl=bool(attrs[2] & termios.HUPCL))


def configure(fd, dtr, rts, clear_hupcl):
    attrs = termios.tcgetattr(fd)
    attrs[0] = attrs[1] = attrs[3] = 0
    attrs[2] = ((attrs[2] & ~(termios.CSIZE | termios.PARENB | termios.CSTOPB))
                | termios.CS8 | termios.CREAD | termios.CLOCAL)
    if clear_hupcl:
        attrs[2] &= ~termios.HUPCL
    attrs[4] = attrs[5] = termios.B115200
    attrs[6][termios.VMIN] = attrs[6][termios.VTIME] = 0
    termios.tcsetattr(fd, termios.TCSANOW, attrs)  # Never flush unread bytes.
    import array
    for policy, bit in ((dtr, termios.TIOCM_DTR), (rts, termios.TIOCM_RTS)):
        if policy == "low":
            fcntl.ioctl(fd, termios.TIOCMBIC, array.array("i", [bit]))


def quarantine(fd, path, duration, maximum):
    """Preserve pre-channel bytes privately; never send them as delivered data."""
    count = 0
    end = time.monotonic() + duration
    out = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        while time.monotonic() < end:
            readable, _, _ = select.select([fd], [], [], max(0, end-time.monotonic()))
            if not readable:
                break
            if count == maximum:
                raise ValueError("startup quarantine extent exhausted; channel not available")
            data = os.read(fd, min(CHUNK, maximum-count))
            if not data:
                raise EOFError("TTY closed during startup quarantine")
            at = 0
            while at < len(data):
                n = os.write(out, data[at:])
                if n <= 0:
                    raise OSError("startup quarantine write made no progress")
                at += n
            count += len(data)
        os.fsync(out)
    finally:
        os.close(out)
        emit("startup-quarantine", bytes=count, delivered=False,
             acknowledged=False, complete=time.monotonic() >= end)


def forward(fd, input_fd, output_fd, idle_seconds, eof_seconds):
    to_device, to_client = bytearray(), bytearray()
    input_open = True
    drain_end = None
    progress = time.monotonic()
    sent = received = 0
    while True:
        now = time.monotonic()
        if now-progress >= idle_seconds:
            raise TimeoutError("byte forwarder idle deadline exceeded")
        if not input_open and not to_device and drain_end is None:
            drain_end = now + eof_seconds
        reading_device = drain_end is None or now < drain_end
        if not reading_device and not to_client:
            return sent, received
        reads = ([input_fd] if input_open and len(to_device) < CHUNK else [])
        if reading_device and len(to_client) < CHUNK:
            reads.append(fd)
        writes = ([fd] if to_device else []) + ([output_fd] if to_client else [])
        wait = min(1.0, idle_seconds-(now-progress))
        if drain_end is not None and reading_device:
            wait = min(wait, max(0, drain_end-now))
        readable, writable, _ = select.select(reads, writes, [], wait)
        for target in writable:
            buffer = to_device if target == fd else to_client
            try:
                n = os.write(target, buffer)
            except BlockingIOError:
                continue  # Backpressure, not a protocol retry or reoffer.
            if n <= 0:
                raise EOFError("forwarded write made no progress")
            del buffer[:n]
            if target == fd:
                sent += n
            else:
                received += n
            progress = time.monotonic()
        for source in readable:
            buffer = to_device if source == input_fd else to_client
            try:
                data = os.read(source, CHUNK-len(buffer))
            except BlockingIOError:
                continue
            if not data:
                if source == input_fd:
                    input_open = False
                    continue
                raise EOFError("physical TTY closed")
            buffer.extend(data)
            progress = time.monotonic()


def run(options, input_fd=0, output_fd=1):
    fd = None
    exclusive = False
    initial = None
    old_flags = {}
    try:
        fd = os.open(options.device, os.O_RDWR | os.O_NOCTTY | os.O_NONBLOCK)
        info = os.fstat(fd)
        if not stat.S_ISCHR(info.st_mode):
            raise ValueError("selected physical path is not a character device")
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        fcntl.ioctl(fd, termios.TIOCEXCL)
        exclusive = True
        mapping = usb_mapping(info.st_rdev)
        owners, unknown = census(info.st_rdev, fd)
        if owners:
            raise ValueError(f"preexisting competing TTY descriptors: {owners}")
        initial = snapshot(fd)
        configure(fd, options.dtr, options.rts, options.clear_hupcl)
        # Unknown privileged descriptors cannot be ruled out by an unprivileged
        # census. Record that limit; never promote this fixture to native Ready.
        emit("opened", linux_boot=Path("/proc/sys/kernel/random/boot_id").read_text().strip(),
             pid=os.getpid(), process_start=Path("/proc/self/stat").read_text().rsplit(")", 1)[1].split()[19],
             fd=fd, device=os.path.realpath(options.device), rdev=[os.major(info.st_rdev), os.minor(info.st_rdev)],
             usb=mapping, initial=initial, configured=snapshot(fd), exclusive_ioctl=True,
             census_complete=unknown == 0, census_unknown=unknown,
             preexisting_visible_openers=owners, physical_peer_authenticated=False)
        if options.startup_capture:
            quarantine(fd, options.startup_capture, options.startup_seconds, options.startup_max_bytes)
        for descriptor in (input_fd, output_fd):
            old_flags[descriptor] = fcntl.fcntl(descriptor, fcntl.F_GETFL)
            fcntl.fcntl(descriptor, fcntl.F_SETFL, old_flags[descriptor] | os.O_NONBLOCK)
        # This is the legacy byte-channel marker, NOT a Conduit Line Ready Sign.
        marker = b"READY\n"
        deadline = time.monotonic()+options.idle_seconds
        while marker:
            if time.monotonic() >= deadline:
                raise TimeoutError("byte-channel marker backpressure deadline")
            _, writable, _ = select.select([], [output_fd], [], max(0, deadline-time.monotonic()))
            if writable:
                try:
                    n = os.write(output_fd, marker)
                except BlockingIOError:
                    continue
                if n <= 0:
                    raise EOFError("byte-channel marker write failed")
                marker = marker[n:]
        sent, received = forward(fd, input_fd, output_fd, options.idle_seconds, options.eof_seconds)
        emit("finished", client_to_tty=sent, tty_to_client=received, acknowledged=False)
        return 0
    except (OSError, ValueError, EOFError, TimeoutError) as error:
        emit("refused", error_type=type(error).__name__, error=str(error)[:512],
             bytes_may_be_in_flight=True, rollback_proved=False)
        return 1
    finally:
        if fd is not None:
            try:
                emit("closing", final=snapshot(fd), exclusive_ioctl=exclusive,
                     reset_free_close_proved=False)
            except OSError:
                emit("closing", final_unknown=True, exclusive_ioctl=exclusive)
            try:
                if exclusive:
                    fcntl.ioctl(fd, termios.TIOCNXCL)
            except OSError:
                emit("cleanup", exclusive_release_unknown=True)
            finally:
                os.close(fd)
        for descriptor, flags in old_flags.items():
            fcntl.fcntl(descriptor, fcntl.F_SETFL, flags)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--device", required=True)
    parser.add_argument("--baud", type=int, choices=[115200], default=115200)
    parser.add_argument("--dtr", choices=["preserve", "low"], default="preserve")
    parser.add_argument("--rts", choices=["preserve", "low"], default="preserve")
    parser.add_argument("--clear-hupcl", action="store_true")
    parser.add_argument("--idle-seconds", type=float, default=300)
    parser.add_argument("--eof-seconds", type=float, default=0.1)
    parser.add_argument("--startup-capture")
    parser.add_argument("--startup-seconds", type=float, default=1)
    parser.add_argument("--startup-max-bytes", type=int, default=16384)
    options = parser.parse_args()
    if not (len(options.device) <= 512 and 0 < options.idle_seconds <= 3600
            and 0 <= options.eof_seconds <= 5 and 0 < options.startup_seconds <= 10
            and 0 < options.startup_max_bytes <= 65536):
        parser.error("proof bounds exceeded")
    return run(options)


if __name__ == "__main__":
    sys.exit(main())
