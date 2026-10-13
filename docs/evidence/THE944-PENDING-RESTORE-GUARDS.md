# THE-944 pending restore and persistence guards — 13 Oct 2026

Partial source correction on Chief-selected engine d252a7a32f027ecc6fad9008106b4f3ae9106732,
branch `feat/the-876-transfer-export`, existing draft PR3. Consumer b377de6 and
RLN f4332a1c pins/artifacts unchanged. No final acceptance or financial release.

## Reproduced defect and product delta

The real active `Wallet::restore_from_snapshot` accepted a legacy snapshot that
erased an existing pending fill. Baseline regression `active_restore_refuses_pending_erasure_replacement_and_state_change`
FAILED (exit101, erasure accepted). The guard now checks the entire typed pending
record before changing sequence, database, BDK or RGB state. It rejects absence,
a different internally consistent fill and both Consented→Applied and
Applied→Consented substitutions. Exact restoration remains allowed; a fresh
engine may restore its existing record. No consent/application authority is
inferred. Legitimate lifecycle transitions need a separate guarded method.

`save_snapshot` now refuses pending intent before opening IndexedDB: plaintext,
sequence-only persistence cannot authenticate the record or exclude stale
higher-sequence writers. It also refuses erasing an existing pending record
before put. `snapshot_for_persistence` and the explicit encrypted encoded-CAS seam
remain available for the consumer-owned secure lifecycle. Automatic legacy saves
cannot acknowledge a pending financial snapshot. Callers must fail/reconcile;
never delete intent or bump a stale writer's sequence to clear this refusal.

Product inventory: `src/wallet/pending_fill.rs` (exact preservation predicate),
`src/wallet/offline.rs` (pre-mutation restore guard, regression matrix),
`src/wallet/idb_store.rs` (legacy-write refusal and regression). This report is
evidence only. No manifest/dependency/protocol/schema/cryptography/artifact change.

## Actual validation and limits

Installed Rust1.97.1, locked/offline:
- Baseline affected active-restore regression: FAIL1/1, exit101.
- Corrected full native suite before final persistence guard: PASS129/129.
- Expanded active-restore matrix: PASS1/1 (six cases, exact restore compatibility).
- Affected `cargo test --lib pending --locked --offline`: PASS9/9.
- Final full `cargo test --lib --locked --offline`: PASS130/130, exit0.
- `cargo check --target wasm32-unknown-unknown --locked --offline`: PASS, type/build check only.
- `cargo fmt --all -- --check`, `git diff --check`: PASS.
- Installed Rust1.98.1 stable `cargo clippy --lib --locked --offline -- -D warnings`: PASS.

Native tests exercise production wallet restore and persistence predicates with
synthetic storage-only records. They do not execute IndexedDB transactions,
consumer AEAD/CAS, current policy/prevout guards, signing, crash or financial
recovery. Existing backup preservation regression remains passing; no claim of
an affected financial backup drill. Prior native/security/driver proof remains
valid at its original revision and level. Changed semantics affect future
active restore, stale/default-writer and pending lifecycle integration.

## Compatibility and remaining work

Chief review now proceeds by comments; the removed native review policy/cap must
not be recreated. This descendant is source work, not an adopted consumer tip.
Saved facade4fb1e8f and pinned f4332a1c were inspected without editing/adoption.
Their default wallet uses `Wallet::flush` (legacy RGB snapshot), whereas the
isolated accounting writer owns encrypted encoded CAS. The secure-store source
explicitly excludes default rgb_lib_wallet IndexedDB snapshots from encryption.
Do not attach that watch-only writer, treat a valid digest as authority, or lift
signing holds. SAME active-default authenticated restore/flush/CAS, guarded
confirm/apply and full serialized bilateral execution remain OPEN. The new
refusal forces correct integration rather than silently routing pending intent
through the legacy path.

Seller spent/change attribution, actual R1/R4 and mandatory live reverse remain
OPEN. R3 remains UNAVAILABLE under Chief's existing disposition; THE-945 held.
THE-872 is still blocked under its operator-control hold; no release retry.
Finite regtest envelope unused. No serving, funding, mining, artifact rebuild,
consumer repin, merge or release. Rollback: revert this source correction before
consumer adoption; doing so restores unsafe pending erasure and legacy writes
and cannot justify activation. No persistent state was migrated.
