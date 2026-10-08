#!/usr/bin/env bash
set -euo pipefail
if [ "$#" -ne 2 ]; then
  echo "usage: $0 GUEST_ISO FRESH_TRANSCRIPT_DIRECTORY" >&2
  exit 2
fi
proof_iso=$(realpath "$1")
proof_transcript=$(realpath -m "$2")
if [ -e "$proof_transcript" ]; then
  echo "refusing to replace an existing transcript directory" >&2
  exit 2
fi
mkdir -p "$proof_transcript"
qemu-system-x86_64 --version > "$proof_transcript/qemu-version.txt"
sha256sum "$proof_iso" > "$proof_transcript/image-sha256.txt"
proof_command=(qemu-system-x86_64 -machine q35 -cpu qemu64 -smp 1 -m 512M \
  -display none -serial "file:$proof_transcript/serial.log" -monitor none \
  -no-reboot -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
  -cdrom "$proof_iso" -boot d)
printf '%q ' "${proof_command[@]}" > "$proof_transcript/command.txt"
printf '\n' >> "$proof_transcript/command.txt"
proof_started=$(date +%s)
set +e
timeout --signal=TERM 1800 "${proof_command[@]}" > "$proof_transcript/stdout.log" 2> "$proof_transcript/stderr.log"
proof_status=$?
set -e
printf 'exit_status=%s\nwall_seconds=%s\n' "$proof_status" "$(( $(date +%s) - proof_started ))" > "$proof_transcript/result.txt"
if [ "$proof_status" -ne 33 ]; then
  echo "guest did not complete successfully; retained transcript: $proof_transcript" >&2
  exit 1
fi
if ! rg -q '^CONDUIT_NUMERIC_PROOF_PASS synthetic-topology-only' "$proof_transcript/serial.log"; then
  echo "guest exit lacked the exact proof receipt" >&2
  exit 1
fi
cat "$proof_transcript/serial.log"
