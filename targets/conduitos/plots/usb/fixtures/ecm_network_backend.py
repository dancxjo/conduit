"""Owned QEMU Ethernet backend for the external ECM device proof fixture."""
import select
import socket
import struct
import subprocess


class NetworkBackend:
    MAX_FRAME = 1514

    def __init__(self, log):
        self.peer, child = socket.socketpair()
        self.peer.setblocking(False)
        command = ['qemu-system-x86_64', '-machine', 'none', '-nodefaults',
                   '-display', 'none', '-monitor', 'none', '-serial', 'none',
                   '-netdev', 'user,id=network,ipv6=off',
                   '-netdev', f'socket,id=wire,fd={child.fileno()}',
                   '-netdev', 'hubport,id=user-port,hubid=0,netdev=network',
                   '-netdev', 'hubport,id=wire-port,hubid=0,netdev=wire']
        try:
            self.process = subprocess.Popen(command, pass_fds=(child.fileno(),),
                                            stdout=log, stderr=log)
        except BaseException:
            self.peer.close()
            raise
        finally:
            child.close()
        self.incoming = bytearray()
        self.outgoing = bytearray()

    def transmit(self, frame):
        if not 14 <= len(frame) <= self.MAX_FRAME:
            raise ValueError('network transmit frame bound')
        if len(self.outgoing) + len(frame) + 4 > 8 * (self.MAX_FRAME + 4):
            raise ValueError('network transmit queue full')
        self.outgoing.extend(struct.pack('!I', len(frame)) + frame)

    def pump(self):
        if self.process.poll() is not None:
            raise EOFError('owned network backend exited')
        readable, writable, _ = select.select([self.peer],
                                              [self.peer] if self.outgoing else [], [], 0)
        if writable:
            sent = self.peer.send(self.outgoing)
            del self.outgoing[:sent]
        if readable:
            data = self.peer.recv(self.MAX_FRAME + 4 - len(self.incoming))
            if not data:
                raise EOFError('owned network backend closed')
            self.incoming.extend(data)
        if len(self.incoming) < 4:
            return None
        length = struct.unpack_from('!I', self.incoming)[0]
        if not 14 <= length <= self.MAX_FRAME:
            raise ValueError('network receive frame bound before allocation')
        if len(self.incoming) < length + 4:
            return None
        frame = bytes(self.incoming[4:length + 4])
        del self.incoming[:length + 4]
        return frame

    def close(self):
        self.peer.close()
        self.process.terminate()
        try:
            self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()
