# Pocket Storage — validation ladder (slice 1)

> **Format changed 2026-11-18.** This file is the `.hnmod` slice's ladder and stands as the
> behaviour evidence. The SDK package's own ladder (runtime tests, fmt/clippy,
> `cargo horizon-mod build`/`check`/`fits`) is in [`notes.md`](notes.md) § *Migration to the
> SDK format*, with its logs in `ports/_shared/evidence/pocketstorage/sdk-*.log`.

Pin `1d63e0a5a37a7a708eb0b32b49bf0dfeb28ed8a9`, mixins
`5a64af938ec86b00d53d16a408aabc0038fb3e57` (schema 6). Every build/test/e2e
ran inside a `ports/tools/build-slot.sh` lease; `CARGO_INCREMENTAL=0`; no
`--target-dir` was passed (the runner's slot cache was used where it applies).

| # | Step | Where | Command | Evidence | Result |
|---|---|---|---|---|---|
| 1 | runtime tests, red | local slot | `cargo test` (before the implementation) | `ports/_shared/evidence/pocketstorage/tdd-red.log` | **exit 101**, 55 compile errors (`Contents`, `Tier`, `tier_of` absent) |
| 2 | runtime tests, green | local slot | `cargo test --locked --offline` | run output | **18 passed**, 0 failed |
| 3 | fmt + clippy | local slot | `cargo fmt --all -- --check && cargo clippy --all-targets --locked --offline -- -D warnings` | run output | clean, 0 warnings |
| 4 | staged plan | remote slot | `python3 tools/horizon.py stage` | run output: `stage_changed: true` | exit 0 — every hook and binding stamped |
| 5 | staged check | remote slot | `python3 tools/horizon.py check` | `evidence/pocketstorage/ladder/horizon-check.log` | `cargo_exit: 0`, **0 warnings** |
| 6 | embedded tests | remote slot | `python3 tools/horizon.py test` | `evidence/pocketstorage/ladder/horizon-test.log` | `test result: ok. 18 passed; 0 failed; 1240 filtered out` |
| 7 | content + bundle | local slot | `tools/package.sh` | `evidence/pocketstorage/ladder/package-*.log` | `.hmod` 3,207 B / 11 entries (`no code: content only`), `.hnmod` 16,885 B / 12 entries, `check` on both exit 0 |
| 8 | e2e, dev mod | remote slot | `build-slot.sh e2e -- tools/e2e.sh` | `evidence/pocketstorage/e2e.log` | **PASS**, `EXIT:0` (phases A–H) |
| 9 | e2e, one file | remote slot | `env EMBEDDED=1 build-slot.sh e2e -- tools/e2e.sh` | `evidence/pocketstorage/e2e-embedded.log` | **PASS**, `EXIT:0`; control leg (no embedded file) knows no id; embedded file byte-identical |
| 10 | e2e, fail-capable | remote slot | `env SELFTEST=1 build-slot.sh e2e -- tools/e2e.sh` | `evidence/pocketstorage/e2e-selftest.log` | **FAIL**, `EXIT:1` |
| 11 | log checker | local | `python3 ports/tools/check-e2e-logs.py pocketstorage --strict` | run output | `logs=3 ok=3`, exit 0 |
| 12 | skills gate | local | `python3 ports/tools/check-skills.py --notes ports/pocketstorage --strict` | run output | `SKILLS GATE: PASS` |
| 13 | package determinism | local | `python3 tools/package.py` twice | `/tmp/opencode/ps-pack{1,2}.json` | byte-identical, `78f4226a…b38e` both times |
| 14 | delivery | local | `cmp pocketstorage.hnmod target/pocketstorage-0.1.0.hnmod` | — | identical |

## Deliverables

```
content/pocketstorage.hmod   3,207 B  36467b325cb4cee40f12a83943231bb3750ecc57087ff9b842e5615debd6b47a   11 entries
pocketstorage.hnmod         16,885 B  78f4226ae1078376d01e7d7d5366df449fd58d1c4bb447b82f43ce21b9ddb38e   12 entries
```

The `.hnmod` holds `horizon-native.json` and `mixins/pocketstorage/`
(`horizon-mixin.toml`, six helpers, `runtime.rs`, `support.rs`, and
`data/pocketstorage-content.hmod`), and its packed sources are byte-identical to
`mixins/pocketstorage/` (checked by `tools/package.py`, which also refuses a
stale content copy).

## Not covered by this ladder

- Crafting the unit in a crafting grid and seeing it in the creative tab: the
  headless harness has no renderer for `ctl click-slot`, so neither is
  exercised (recorded in `notes.md`).
- A second game binary/OS: the e2e ran on the second build machine (Linux, the
  same pin), headless.
