"""Linux external-fixture children cannot outlive their owning wrapper.

The wrapper is single-threaded. This pre-exec hook only installs a kernel
parent-death signal and checks the fork/installation race, before exec.
It is host probe supervision, never native transfer cancellation evidence.
"""
import ctypes
import os
import signal
import sys


def parent_death_guard():
    if sys.platform != 'linux':
        raise RuntimeError('bounded BOT fixture ownership requires Linux parent-death signals')
    parent = os.getpid()
    libc = ctypes.CDLL(None, use_errno=True)
    prctl = libc.prctl
    prctl.argtypes = [ctypes.c_int, ctypes.c_ulong, ctypes.c_ulong,
                     ctypes.c_ulong, ctypes.c_ulong]
    prctl.restype = ctypes.c_int

    def before_exec():
        if prctl(1, signal.SIGTERM, 0, 0, 0) != 0:  # PR_SET_PDEATHSIG
            os._exit(127)
        if os.getppid() != parent:
            os._exit(126)

    return before_exec
