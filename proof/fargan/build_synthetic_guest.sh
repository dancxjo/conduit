#!/usr/bin/env bash
# Local synthetic resources only; no downloads or voice/model admission.
set -euo pipefail
if [ "$#" -ne 3 ]; then
  echo "usage: $0 FIXTURE_DIRECTORY OUTPUT_DIRECTORY PINNED_LIMINE_ARCHIVE" >&2
  exit 2
fi
proof_root=$(cd "$(dirname "$0")/../.." && pwd)
proof_fixture=$(realpath "$1")
proof_output=$(realpath -m "$2")
proof_archive=$(realpath "$3")
printf '%s  %s\n' 4c760c09c53560d859b362319a3dc63b79cca3d47f35d69ab0106a13b8057055 "$proof_archive" | sha256sum -c -
if [ -e "$proof_output/conduitos" ] || [ -e "$proof_output/conduitos.iso" ]; then
  echo "refusing to replace an existing guest artifact; select a fresh output directory" >&2
  exit 2
fi
mkdir -p "$proof_output"
proof_stage=$(mktemp -d "$proof_output/image-stage-XXXXXXXX")
tar -xzf "$proof_archive" -C "$proof_stage"
proof_limine="$proof_stage/limine-binary"
make -C "$proof_limine"
proof_target=${CARGO_TARGET_DIR:-"$proof_root/target"}
cd "$proof_root"
CONDUITOS_NUMERIC_FIXTURE_DIR="$proof_fixture" \
CONDUITOS_MAKE_RECORD="$proof_root/proof/fargan/synthetic_guest_make.rs" \
CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_PROFILE_RELEASE_DEBUG=0 \
CARGO_PROFILE_RELEASE_OPT_LEVEL=1 CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16 \
RUSTFLAGS='-C relocation-model=static -C panic=abort --cfg curve25519_dalek_backend="serial" --cfg aes_force_soft --cfg polyval_force_soft --cfg chacha20_force_soft --cfg poly1305_force_soft' \
cargo +stable build --locked -p conduitos --bin conduitos \
  --target x86_64-unknown-none --release --no-default-features \
  --features numeric-topology-catalog-cache
proof_kernel="$proof_target/x86_64-unknown-none/release/conduitos"
cp "$proof_kernel" "$proof_output/conduitos"
mkdir -p "$proof_stage/iso/boot/limine" "$proof_stage/iso/EFI/BOOT"
cp "$proof_kernel" "$proof_stage/iso/boot/conduitos"
for proof_name in limine-bios.sys limine-bios-cd.bin limine-uefi-cd.bin; do
  cp "$proof_limine/$proof_name" "$proof_stage/iso/boot/limine/$proof_name"
done
cp "$proof_limine/BOOTX64.EFI" "$proof_stage/iso/EFI/BOOT/BOOTX64.EFI"
cat > "$proof_stage/iso/limine.conf" <<'CONFIG'
timeout: 0
serial: yes
/ConduitOS synthetic numeric proof
    protocol: limine
    path: boot():/boot/conduitos
    cmdline: profile=conduitos/synthetic-numeric-topology@1
CONFIG
xorriso -as mkisofs -b boot/limine/limine-bios-cd.bin -no-emul-boot \
  -boot-load-size 4 -boot-info-table --efi-boot boot/limine/limine-uefi-cd.bin \
  -efi-boot-part --efi-boot-image --protective-msdos-label \
  "$proof_stage/iso" -o "$proof_output/conduitos.iso"
"$proof_limine/limine" bios-install "$proof_output/conduitos.iso"
sha256sum "$proof_output/conduitos" "$proof_output/conduitos.iso" > "$proof_output/image-sha256.txt"
size "$proof_output/conduitos" > "$proof_output/elf-sections.txt"
