# Conduit repository task menu.
# All orchestration logic lives in `cargo xtask`.
# This file is a thin alias layer for developer convenience.

default:
    @just --list

# ── Primary families ─────────────────────────────────────────────────────────

# Run a check suite (default: all).
check suite="all" *args:
    cargo xtask check {{suite}} {{args}}

# Run a demo (default: std).
demo name="std" *args:
    cargo xtask demo {{name}} {{args}}

# Run a proof.
prove name *args:
    cargo xtask prove {{name}} {{args}}

# Inspect prerequisites (default: all).
doctor target="all" *args:
    cargo xtask doctor {{target}} {{args}}

# ── Compatibility aliases (muscle-memory and migration wrappers) ──────────────

std-host:
    cargo xtask demo std

demo-std:
    cargo xtask demo std

std:
    cargo xtask demo std

demo-triple-local:
    cargo xtask demo triple-local

browser-sim:
    cargo xtask check simulation

browser-frame-fixture:
    cargo test -p conduit-browser-sim std_host_sends_signal_to_browser_through_bounded_frame_fixture

triple-sim-proof:
    cargo test -p conduit-browser-sim triple_signal_form_fans_out_to_std_and_simulated_receipts

browser-sim-wasm-check:
    cargo check -p conduit-browser-sim --target wasm32-unknown-unknown

pico-sim:
    cargo test -p conduit-pico-sim

pico-datagram-fixture:
    cargo test -p conduit-pico-sim std_host_sends_signal_to_pico_through_bounded_datagram_fixture

pico-sim-thumb-check:
    cargo check -p conduit-pico-sim --no-default-features --target thumbv6m-none-eabi

realm:
    cargo xtask check realm

realm-thumb-check:
    cargo check -p conduit-realm --target thumbv6m-none-eabi

observatory:
    cargo xtask check observatory

observatory-thumb-check:
    cargo check -p conduit-observatory --target thumbv6m-none-eabi

std-catalog:
    cargo xtask check std-catalog

std-catalog-thumb-check:
    cargo check -p conduit-std-catalog --no-default-features --target thumbv6m-none-eabi

prove-std-browser-s4:
    cargo xtask prove std-browser-s4

std-browser:
    cargo xtask prove std-browser

check-kernel-s1:
    cargo xtask check kernel-s1

check-kernel-takeover:
    cargo xtask check kernel-takeover

check-planning-s2:
    cargo xtask check planning-s2

check-form-s3:
    cargo xtask check form-s3

check-browser-s4:
    cargo xtask check browser-s4

check-realm-readiness:
    cargo xtask check realm

check-observatory-readiness:
    cargo xtask check observatory

check-std-catalog-readiness:
    cargo xtask check std-catalog

check-sim-readiness:
    cargo xtask check simulation
