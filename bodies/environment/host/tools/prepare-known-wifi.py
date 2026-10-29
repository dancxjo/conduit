#!/usr/bin/env python3
"""Prepare private MCU Wi-Fi profiles; never change NetworkManager or any NIC.

Run in a terminal: prepare-known-wifi.py --add-ssid Troys
Passwords never enter argv, shell commands, Source, or diagnostic output.
"""
import argparse
import ctypes
import errno
import getpass
import json
import os
from pathlib import Path
import re
import secrets
import stat
import subprocess
import sys
import warnings

SCHEMA = "conduit.private/known-wifi@1"
MAX_PROFILES = 4


class PreparationError(Exception):
    """A deliberately nonsecret diagnostic suitable for the terminal."""


def validate_ssid(value):
    try:
        size = len(value.encode("utf-8"))
    except (AttributeError, UnicodeError):
        raise PreparationError("SSID must be valid UTF-8") from None
    if not 1 <= size <= 32 or "\0" in value:
        raise PreparationError("SSID must contain 1–32 UTF-8 bytes and no NUL")
    return value


def validate_password(value):
    if not isinstance(value, str) or not 8 <= len(value) <= 63 or any(
        not 0x20 <= ord(char) <= 0x7E for char in value
    ):
        raise PreparationError("Password must contain 8–63 printable ASCII characters")
    return value


def _run(arguments):
    try:
        return subprocess.run(
            arguments, capture_output=True, text=True, check=False, timeout=8
        )
    except (OSError, UnicodeError, subprocess.SubprocessError):
        # Never include command stdout/stderr: --show-secrets may contain a PSK.
        raise PreparationError("Read-only NetworkManager query failed") from None


def active_wifi(run=_run):
    result = run([
        "nmcli", "--terse", "--escape", "no", "--fields", "UUID,TYPE",
        "connection", "show", "--active",
    ])
    if result.returncode:
        raise PreparationError("Cannot read the active NetworkManager connection")
    wireless = []
    for line in result.stdout.splitlines():
        uuid, separator, kind = line.partition(":")
        if separator and kind in ("802-11-wireless", "wifi"):
            if not re.fullmatch(r"[0-9a-fA-F]{8}(?:-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}", uuid):
                raise PreparationError("Active Wi-Fi UUID is malformed")
            wireless.append(uuid)
    if len(wireless) != 1:
        raise PreparationError("Exactly one active NetworkManager Wi-Fi connection is required")
    uuid = wireless[0]
    result = run([
        "nmcli", "--escape", "no", "--get-values", "802-11-wireless.ssid",
        "connection", "show", "uuid", uuid,
    ])
    if result.returncode:
        raise PreparationError("Cannot read the active Wi-Fi SSID")
    ssid = validate_ssid(result.stdout.removesuffix("\n"))
    # This query is read-only and uses the existing user's keyring permissions.
    # No sudo, agent registration, or NetworkManager secret mutation is used.
    try:
        result = run([
            "nmcli", "--show-secrets", "--escape", "no", "--get-values",
            "802-11-wireless-security.psk", "connection", "show", "uuid", uuid,
        ])
    except PreparationError:
        return ssid, None
    password = result.stdout.removesuffix("\n") if result.returncode == 0 else None
    try:
        return ssid, validate_password(password)
    except PreparationError:
        return ssid, None


def secure_prompt(prompt):
    # getpass otherwise warns and may fall back to echoing on stdin. Refuse it.
    with warnings.catch_warnings():
        warnings.simplefilter("error", getpass.GetPassWarning)
        try:
            return getpass.getpass(prompt, stream=sys.stderr)
        except (getpass.GetPassWarning, EOFError):
            raise PreparationError("A private interactive terminal is required") from None


def read_password_file(path, uid=None):
    """Read one explicit user's raw password file, never following a symlink."""
    uid = os.getuid() if uid is None else uid
    descriptor = None
    data = bytearray()
    try:
        descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode) or before.st_uid != uid or before.st_mode & 0o022:
            raise PreparationError("Password file must be an owned regular file without shared write access")
        if before.st_size > 1024:
            raise PreparationError("Password file exceeds the 1024-byte input bound")
        while len(data) <= 1024:
            chunk = os.read(descriptor, 1025 - len(data))
            if not chunk:
                break
            data.extend(chunk)
            if len(data) > 1024:
                raise PreparationError("Password file exceeds the 1024-byte input bound")
        after = os.fstat(descriptor)
        if (before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (after.st_size, after.st_mtime_ns, after.st_ctime_ns):
            raise PreparationError("Password file changed during the read")
        # Remove exactly one line terminator, preserving meaningful spaces.
        if data.endswith(b"\r\n"):
            del data[-2:]
        elif data.endswith((b"\n", b"\r")):
            del data[-1:]
        return validate_password(data.decode("ascii"))
    except PreparationError:
        raise
    except (OSError, UnicodeError, ValueError):
        raise PreparationError("Cannot safely read the password file") from None
    finally:
        data[:] = b"\0" * len(data)
        if descriptor is not None:
            os.close(descriptor)


def collect_profiles(add_ssids, run=_run, prompt=secure_prompt, password_file=None, read_password=read_password_file):
    requested = [validate_ssid(ssid) for ssid in add_ssids]
    if password_file is not None and len(requested) != 1:
        raise PreparationError("--password-file requires exactly one --add-ssid")
    supplied = validate_password(read_password(password_file)) if password_file is not None else None
    current, saved_password = active_wifi(run)
    if len(set([current, *requested])) > MAX_PROFILES:
        raise PreparationError("At most four distinct Wi-Fi profiles are supported")
    profiles = []

    def upsert(ssid, password):
        profile = {
            "ssid": ssid,
            "password": validate_password(password),
            "security": "wpa2-personal",
            "priority": 0,
        }
        for index, previous in enumerate(profiles):
            if previous["ssid"] == ssid:
                profiles[index] = profile
                return
        profiles.append(profile)

    def ask(ssid):
        # JSON quoting keeps control bytes in an SSID out of terminal control.
        return prompt("Wi-Fi password for SSID " + json.dumps(ssid, ensure_ascii=True) + ": ")

    if saved_password is None and supplied is not None and current == requested[0]:
        saved_password = supplied
    upsert(current, saved_password if saved_password is not None else ask(current))
    for ssid in requested:
        upsert(ssid, supplied if supplied is not None else ask(ssid))
    return {"schema": SCHEMA, "profiles": profiles}


def _rename_noreplace(directory, source, destination):
    # Linux renameat2 gives atomic no-clobber publication, unlike a check followed
    # by os.rename. Failure never falls back to an overwriting rename.
    libc = ctypes.CDLL(None, use_errno=True)
    rename = getattr(libc, "renameat2", None)
    if rename is None:
        raise PreparationError("Atomic no-overwrite rename is unavailable")
    rename.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    rename.restype = ctypes.c_int
    if rename(directory, os.fsencode(source), directory, os.fsencode(destination), 1) != 0:
        number = ctypes.get_errno()
        if number == errno.EEXIST:
            raise PreparationError("Output already exists; use --overwrite explicitly")
        raise PreparationError("Atomic private output publication failed")


def _open_private_destination(output, overwrite, uid):
    output = Path(output)
    if not output.is_absolute() or output.name in ("", ".", ".."):
        raise PreparationError("Output must be an absolute file path")
    directory = None
    try:
        directory = os.open(output.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        parent = os.fstat(directory)
        if parent.st_uid != uid or parent.st_mode & 0o077:
            raise PreparationError("Output parent must be an owned private directory (0700)")
        try:
            existing = os.stat(output.name, dir_fd=directory, follow_symlinks=False)
        except FileNotFoundError:
            existing = None
        if existing is not None:
            if not stat.S_ISREG(existing.st_mode) or existing.st_uid != uid or stat.S_IMODE(existing.st_mode) != 0o600:
                raise PreparationError("Existing output must be an owned regular private file (0600)")
            if not overwrite:
                raise PreparationError("Output already exists; use --overwrite explicitly")
        return output, directory
    except Exception:
        if directory is not None:
            os.close(directory)
        raise


def write_private(document, output, overwrite=False, uid=None):
    uid = os.getuid() if uid is None else uid
    directory = None
    temporary = None
    try:
        output, directory = _open_private_destination(output, overwrite, uid)
        temporary = ".conduit-known-wifi." + secrets.token_hex(12) + ".tmp"
        descriptor = os.open(
            temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
            0o600, dir_fd=directory,
        )
        os.fchmod(descriptor, 0o600)
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            json.dump(document, handle, ensure_ascii=False, separators=(",", ":"))
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        if overwrite:
            os.replace(temporary, output.name, src_dir_fd=directory, dst_dir_fd=directory)
        else:
            _rename_noreplace(directory, temporary, output.name)
        temporary = None
        os.fsync(directory)
    except PreparationError:
        raise
    except (OSError, ValueError, TypeError):
        raise PreparationError("Cannot safely publish the private Wi-Fi file") from None
    finally:
        if directory is not None:
            if temporary is not None:
                try:
                    os.unlink(temporary, dir_fd=directory)
                except FileNotFoundError:
                    pass
            os.close(directory)


def main(argv=None, *, run=_run, prompt=secure_prompt, interactive=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--add-ssid", action="append", default=[], help="additional SSID; its password is prompted privately")
    parser.add_argument("--password-file", help="owned raw password file for exactly one --add-ssid; no extra password prompt")
    parser.add_argument("--output", default=f"/run/user/{os.getuid()}/conduit-known-wifi.json")
    parser.add_argument("--overwrite", action="store_true", help="explicitly replace an existing owned 0600 file")
    args = parser.parse_args(argv)
    if interactive is None:
        interactive = sys.stdin.isatty() and sys.stderr.isatty()
    try:
        if not interactive:
            raise PreparationError("Run this helper in an interactive terminal; passwords must not be piped")
        # Reject a bad destination or missing overwrite authorization before
        # querying a secret or asking the user to enter one. Recheck on publish.
        try:
            _, directory = _open_private_destination(args.output, args.overwrite, os.getuid())
        except OSError:
            raise PreparationError("Cannot safely open the private output directory") from None
        os.close(directory)
        document = collect_profiles(args.add_ssid, run, prompt, args.password_file)
        write_private(document, args.output, args.overwrite)
        print(f"Saved {len(document['profiles'])} private Wi-Fi profile(s) to {args.output}")
        return 0
    except (PreparationError, KeyboardInterrupt) as error:
        message = str(error) if isinstance(error, PreparationError) else "Cancelled; no credentials were published"
        print(message, file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
