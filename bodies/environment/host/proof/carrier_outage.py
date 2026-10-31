"""Attended real-MCU carrier recovery through a transparent local fault relay.

Requires a fresh, prepared MCU Boot and an already lulled Body biography.
Drops an actual signed EvidenceAccepted reply after Root applies acceptance,
refuses management connections for 150 seconds, and checks same-Boot history
and display recovery without lifecycle replay. The owner and relay stay running
on success. This is TCP-loss HIL, not a Wi-Fi association or cold-Boot proof.
"""
import asyncio
import ipaddress
import json
import os
import signal
import sys
import time
import urllib.request
from pathlib import Path
from urllib.parse import urlparse
sys.dont_write_bytecode = True
from acceptance_loss import SingleAcceptanceLoss, verify_lost_receipt

upstream, state, resume, executable, output = sys.argv[1:]
parsed = urlparse(upstream)
assert parsed.scheme == "tcp" and parsed.port and parsed.username is None and parsed.password is None
assert parsed.path in ["", "/"] and not parsed.query and not parsed.fragment
address = ipaddress.ip_address(parsed.hostname)
state = Path(state).resolve()
resume = Path(resume).resolve()
assert json.loads(resume.read_bytes())["body"]["state"] == "Lulled"
output = Path(output).resolve()
output.mkdir(mode=0o700)
owner_log = output / "owner.log"
expected_body = json.loads(resume.read_bytes())["body_id"]
http_host = f"[{address}]" if address.version == 6 else str(address)
http = f"http://{http_host}"


def emit(event, **values):
    print(json.dumps({"event": event, **values}), flush=True)


def rows(path):
    raw = path.read_bytes()
    raw = raw[: raw.rfind(b"\n") + 1]
    return [json.loads(line) for line in raw.splitlines() if line]


def get(route):
    with urllib.request.urlopen(http + route, timeout=4) as response:
        raw = response.read(2049)
    assert len(raw) <= 2048
    return json.loads(raw)


def run_for(pid):
    paths = []
    for fd in Path(f"/proc/{pid}/fd").iterdir():
        try:
            path = fd.readlink()
        except FileNotFoundError:
            continue
        if path.name == "native-observations.ndjson" and path.parent.parent == state / "runs":
            paths.append(path.parent)
    assert len(paths) == 1, paths
    return paths[0]




async def main():
    loss = SingleAcceptanceLoss()
    blackout = False
    lost = asyncio.Event()
    lost_reply = None

    async def accept(reader, writer):
        nonlocal blackout, lost_reply
        if blackout:
            writer.close()
            await writer.wait_closed()
            return
        try:
            upstream_reader, upstream_writer = await asyncio.wait_for(asyncio.open_connection(str(address), parsed.port), timeout=5)
        except (OSError, TimeoutError):
            writer.close()
            await writer.wait_closed()
            return

        async def copy(source, destination, inspect=False):
            nonlocal blackout, lost_reply
            pending = b""
            while chunk := await source.read(4096):
                if inspect:
                    pending += chunk
                    while b"\n" in pending:
                        line, pending = pending.split(b"\n", 1)
                        try:
                            reply = json.loads(line)
                        except (ValueError, UnicodeDecodeError):
                            continue
                        if loss.should_drop(reply):
                            lost_reply = reply
                            blackout = True
                            # Withhold this chunk, including the receipt's end.
                            # The owner cannot have received a complete reply.
                            lost.set()
                            return
                    if len(pending) > 8192:
                        pending = pending[-8192:]
                destination.write(chunk)
                await destination.drain()

        tasks = [asyncio.create_task(copy(reader, upstream_writer)), asyncio.create_task(copy(upstream_reader, writer, True))]
        try:
            await asyncio.wait(tasks, return_when=asyncio.FIRST_COMPLETED)
        finally:
            for task in tasks:
                task.cancel()
            await asyncio.gather(*tasks, return_exceptions=True)
            for stream in [writer, upstream_writer]:
                stream.close()
                try:
                    await stream.wait_closed()
                except OSError:
                    pass

    server = await asyncio.start_server(accept, "127.0.0.1", 0)
    port = server.sockets[0].getsockname()[1]
    with owner_log.open("xb") as log:
        os.fchmod(log.fileno(), 0o600)
        owner = await asyncio.create_subprocess_exec(
            executable, f"tcp://127.0.0.1:{port}", str(state), "960", f"--resume={resume}",
            f"--history-peer={http_host}:80", "--display-check", stdout=log, stderr=log, start_new_session=True,
        )
        emit("carrier-proof-owner-started", pid=owner.pid, relay=f"127.0.0.1:{port}", output=str(output))
        run = None
        failure = None
        try:
            deadline = time.monotonic() + 180
            while time.monotonic() < deadline:
                assert owner.returncode is None, owner.returncode
                if any(row.get("event") == "verified-observation-history" for row in rows(owner_log)):
                    run = run_for(owner.pid)
                    break
                await asyncio.sleep(0.25)
            assert run is not None, "No initial verified measurement/history"
            admitted = json.loads((run / "mcu-admission.json").read_bytes())
            boot = admitted["reply"]["boot_id"]
            assert admitted["reply"]["body_id"] == expected_body
            loss.armed = True
            await asyncio.wait_for(lost.wait(), timeout=30)
            invocation = verify_lost_receipt(run, lost_reply)
            with (output / "dropped-verified-receipt.json").open("xb") as receipt_file:
                os.fchmod(receipt_file.fileno(), 0o600)
                receipt_file.write(json.dumps(lost_reply).encode())
                receipt_file.flush()
                os.fsync(receipt_file.fileno())
            directory = os.open(output, os.O_RDONLY)
            os.fsync(directory)
            os.close(directory)
            history_before = get("/history?after=0")["committed"]
            before = get("/reading")
            emit("carrier-proof-reply-lost", pid=owner.pid, run=str(run), boot=boot, invocation=invocation,
                 command_sequence=lost_reply["sequence"], accepted=lost_reply["local_evidence"]["accepted"], sample=before["sequence"])
            for _ in range(3):
                await asyncio.sleep(50)
                current = get("/reading")
                assert owner.returncode is None
                emit("carrier-proof-blackout", sample=current["sequence"], blackout=blackout)
            after = get(f"/history?after={history_before}")
            assert after["committed"] >= history_before + 2
            blackout = False
            emit("carrier-proof-restored", required_through=after["committed"])
            deadline = time.monotonic() + 240
            while time.monotonic() < deadline:
                assert owner.returncode is None
                log_rows = rows(owner_log)
                recovered = [r for r in log_rows if r.get("event") == "owner-link-recovered"]
                collected = [r["through_sequence"] for r in log_rows if r.get("event") == "verified-observation-history"]
                if recovered and max(collected, default=0) >= after["committed"]:
                    assert all(r["boot"] == boot for r in recovered)
                    display = rows(run / "display-control.ndjson")
                    assert not any(r["request"]["sequence"] > lost_reply["sequence"] and r["request"]["operation"] in ["Reserve", "Prepare", "Start", "Release"] for r in display)
                    native = rows(run / "native-control.ndjson")
                    assert not any(r["request"]["sequence"] > lost_reply["sequence"] for r in native)
                    latest = max(display, key=lambda r:r["request"]["sequence"])["reply"]
                    if latest["local_evidence"]["accepted"] >= lost_reply["local_evidence"]["committed"] + 40:
                        emit("carrier-proof-passed", pid=owner.pid, run=str(run), boot=boot, recovered=recovered[-1],
                             history_through=max(collected), display=latest["local_evidence"], job_restarted=False)
                        break
                await asyncio.sleep(1)
            else:
                raise RuntimeError("Recovery did not complete in its finite window")
        except Exception as error:
            failure = error
            emit("carrier-proof-failed", error=str(error), owner_alive=owner.returncode is None)
        finally:
            # A failed proof restores its relay too; never strand a running job
            # behind the injected blackout. Keep the relay alive for the owner.
            loss.armed = False
            blackout = False
        emit("carrier-relay-retained", pid=owner.pid, port=port, run=str(run) if run else None)
        code = await owner.wait()
        server.close()
        await server.wait_closed()
        assert code == 0, code
        if failure is not None:
            raise failure

asyncio.run(main())
