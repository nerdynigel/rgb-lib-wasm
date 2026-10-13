# THE-944 explicit persistence failure correction — 13 Oct 2026

Partial necessary source correction, based on Chief-selected engine
`1720c05d7db744c00d84b87c8a43278fad4a932c`, existing branch
`feat/the-876-transfer-export`, [draft PR3](https://github.com/nerdynigel/rgb-lib-wasm/pull/3).
Consumer pins/artifacts remain unchanged. No financial operation or browser invocation.

## Defect and change

Source inspection found `save_snapshot` returned `Ok(())` when the existing
sequence was equal/newer. Therefore `Wallet::flush` could acknowledge a snapshot
it never committed. Native IndexedDB reproduction is unavailable: this run did
not execute browser storage. The production stale-write predicate now returns
an explicit persistence error before any put. Sequence allocation uses checked
atomic update: exhaustion refuses without wrapping or changing the counter.
Callers must reconcile stale state, not increase a stale writer's counter to retry.

Product inventory: `src/wallet/idb_store.rs` (stale refusal and regression),
`src/wallet/offline.rs` (checked sequence and exhaustion regression, flush contract),
`src/wallet/online.rs` (three previously reported clippy diagnostics corrected,
identical values/error propagation). This document is evidence only. No dependency,
protocol, request/digest/schema or generated artifact changes.

## Executed validation

- Rust1.97.1 `cargo test --lib snapshot --locked --offline`: PASS7/7.
- Rust1.97.1 `cargo test --lib --locked --offline`: PASS128/128, exit0.
- Rust1.97.1 `cargo check --target wasm32-unknown-unknown --locked --offline`: PASS,
  type/build check only; no generated deployable WASM artifact.
- Rust1.97.1 `cargo fmt --all -- --check`, `git diff --check`: PASS.
- `cargo +1.97.1 clippy`: BLOCKED, installed toolchain lacks clippy component.
- Existing Rust1.98.1 (`+stable`) `cargo clippy --lib --locked --offline -- -D warnings`:
  PASS, exit0. No toolchain installation/settings change or diagnostic suppression.

Two added regressions exercise production stale-write/sequence logic; they do not
prove an IndexedDB transaction or authenticated CAS. Previously published native
126/126 and security/source evidence remain valid at their original revisions and
levels. Affected future proof: explicit flush refusal, default-engine failed-writer
handling and authenticated financial persistence/recovery. Historical driver success
is not served product proof.

## Authority, compatibility and next implementation boundary

Chief's exact1720c05d source-selection decision is retained. This descendant has NOT
been selected as a consumer pin, rebuilt, served or activated. Legacy snapshots and
immutable pending record bytes remain compatible. Stale writes now return failure;
callers depending on false success must fail/reconcile instead. Monotonic sequence
comparison alone cannot prevent an old wallet with a larger sequence from overwriting
newer state: SAME active engine authenticated compare-and-swap remains required.

Inspected saved facade4fb1e8f and published consumer f4332a1c without adopting/overwriting
the saved checkout. Its isolated accounting writer is separate from the default wallet;
no writer attachment or signing hold was changed. Necessary remaining source work is
active-default guarded confirm/apply/restore/flush/authenticated CAS and serialized
bilateral execution. R1/R4, seller per-UTXO attribution and required live reverse are
OPEN; R3 UNAVAILABLE. THE-945 remains held. Finite financial envelope unused.

Rollback: revert this source commit before artifact adoption. That restores the
incorrect stale-success behavior and must not be used to justify financial activation.
No persisted migration/economic effects occurred. Do not downgrade future pending
snapshots to a schema that drops the exact intent.
