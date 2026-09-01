# AGENTS.md

Hardware product monorepo: ESP32-S3 firmware (Rust/esp-rs), KiCad PCB, schematic-generator tooling, product docs. **Pre-POC**: firmware is feature-complete, the PCB is schematic-only (unrouted), and the physics POC has never been built. Live status is on GitHub — see *Project management*.

## Commands

All firmware commands run from `firmware/` via `just` — see `firmware/justfile`:

| Task | Command |
|------|---------|
| Host tests | `just test` |
| Single test | `just test-one <test_name>` |
| Lint / format | `just lint` / `just fmt` |
| ESP32-S3 build | `just build` |
| Flash device | `just flash` |
| Local CI | `just ci` (fmt + lint + test + audit) |
| QEMU boot check | `just qemu` |
| Fuzz target | `just fuzz <target>` (nightly) |

- Host test/lint **must** use `--no-default-features --target x86_64-unknown-linux-gnu` — default features pull in `espidf`, which fails without the xtensa toolchain. CI runs exactly this command; never push with it red.
- ESP builds need the `esp` rustup toolchain and `-Z build-std=std,panic_abort`.

## Dual-target gotcha (highest-frequency breakage)

Everything compiles twice: `espidf` (device) and host (`--no-default-features`). Every adapter has `#[cfg(target_os = "espidf")]` / `#[cfg(not(...))]` halves — **each cfg branch needs its own imports and constants**. A rename on one branch silently breaks the other until compile; this produced 4 compile errors on main (issue #13). When touching an adapter, check both targets (`just test`, and `just build` or `cargo +stable check --no-default-features --target x86_64-unknown-linux-gnu`).

## Codegen

`firmware/build.rs` regenerates `src/rpc/generated/petfilter_generated.rs` from `schemas/petfilter.fbs` via flatc on every cargo invocation.
- Never hand-edit the generated file.
- flatc version matters: codegen expects **25.12.19**, CI pins **25.1.21** (issue #19).
- Running cargo can dirty the worktree with regenerated bindings — check `git status` after builds; never commit regenerations from a mismatched flatc.

## Architecture (non-obvious)

- Hexagonal: `app/`, `fsm/`, `control/`, `scheduler.rs` are pure, host-testable logic. Side effects live in `adapters/` (WiFi, BLE, NVS, TLS, mDNS, cert store, time) and `drivers/`, behind traits in `app/ports.rs`.
- Single event loop in `main.rs`: SPSC event queue + FreeRTOS task notifications (event types in `events.rs`).
- C ULP component (`ulp_nh3_component/`) monitors NH3 during deep sleep; shared RTC state in `power.rs`.

## Comms constraint (architecture law)

RPC-only: FlatBuffers over TLS 1.3-PSK (TCP `:4242`) and BLE GATT provisioning. **No webUI, REST, MQTT, captive portal, or AP mode — do not add them.**
Auth: HMAC-SHA256 challenge/response (`rpc/auth.rs`), replay protection (monotonic seq), token-bucket rate limiting.
Known security gap (issue #14): the RPC PSK is hardcoded in `main.rs` (`default-psk-change-me`); the BLE-provisioned PSK is stored to NVS but never loaded. Do not copy this pattern into new code.

## Hardware

- KiCad 8 project in `hardware/pcb/`. Schematic sheets and symbols are **generated** by `tools/*.py` (`kicad_schgen.py`, `kicad_libgen.py`, `gen_schematics.py`) — regenerate, don't hand-edit.
- PCB layout is empty (0 footprints, issue #17); the power sheet has open ERC errors (issue #16). ERC/DRC/BOM/gerbers via KiBot in Docker: `make validate`, `make gerbers` (see `hardware/pcb/Makefile`).
- `docs/engineering/` holds the subsystem specs (venturi, UVC, power, noise) — consult before electrical changes.

## Project management (required workflow)

GitHub issues + milestones are the single status source:

- Issues #1–#23 under milestones **Gate 1: POC-Ready Firmware**, **Gate 2: Prototype Greenlight**, **Gate 3: Pre-Production**.
- Label taxonomy: priority `P0`–`P3`; area `area:firmware|hardware|client|infra|docs`; kind `kind:security|hardening|wiring|spec`; plus `gate`.
- Semantics: **priority label = urgency; milestone = which gate the work serves.**
- Dedup against existing issues before creating; assign `@me`.
- Close an issue only with a file:line evidence comment (repo convention).
- Gate discipline: the bench POC (issue #15, >80% ammonia reduction in a sealed chamber) is the project's binary go/no-go. **Never recommend or approve spending** — funding is gate-released, and the POC budget is exactly EUR 1,500.

## unlazy skill (required for multi-step work)

Before any multi-step task, write acceptance gates to a `GATES.md` (in the working dir or `/tmp/opencode/<task>/`): one checkbox per required outcome, each with a runnable `CHECK:` command, an `EXPECT:` line, and an `EVIDENCE:` line (never left `pending`). Finish only when every box has evidence, verified by:

```
node ~/.agents/skills/unlazy/scripts/gate-check.mjs <dir-with-GATES.md>
```

No final report while any box is unchecked. If a gate becomes impossible, record `ABANDON: <id> <reason>` instead of dropping it silently. Solo mode under ~30 min of work; decompose into `PLAN.md` + per-leaf gates files for larger builds.

## Documentation trust map

- `docs/firmware/work-breakdown.md` — **historical plan** (status banner at top): all P0-1…P2-8 tasks are implemented; do not use it as a status source.
- `docs/research/technology-validation.md` — physics validation reference; POC results get appended here (issue #15).
- `docs/engineering|business|branding|research/` — durable reference docs.
- `README.md` — current; `just ci` before push.

## Gotchas

- `.opencode/` is untracked (local OpenCode tooling only) — don't commit it.
- `cargo audit` and coverage are currently soft CI gates (issue #20) — treat their failures as real anyway.
- Verify host compilation locally before any push; `main` may be red (issue #13) until it lands.
