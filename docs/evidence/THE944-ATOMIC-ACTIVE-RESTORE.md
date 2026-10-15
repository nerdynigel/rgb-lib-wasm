# THE-944 active restore failure atomicity — 13 Oct 2026

PARTIAL source correction on `feat/the-876-transfer-export`, existing draft PR3.
Chief selected f885c3fe010910ad5493102dd632ceef191c0128 for continued source
integration on 13 Oct at 11:03 UTC, retaining d252a7a ancestry. Consumer engine
b377de6aec807adb597e7f0bced7fa578a54713b and RLN
f4332a1c56d1d16682996fb717ecfaf4b4400cd4 pins/artifacts remain unchanged.
No consumer adoption, financial authorization or final acceptance.

## Reproduced failure and correction

`Wallet::restore_from_snapshot` changed the sequence, pending record, RGB database
and BDK store before validating the supplied BDK wallet and RGB stock. Baseline
`failed_active_restore_preserves_complete_wallet_state` FAILED (exit101): invalid
BDK state returned an error after adopting sequence100, a new pending record and
an empty BDK changeset. This is an executed native synthetic-state failure, not
an observed live financial loss.

Restore now loads BDK into a separate temporary MemoryStore, validates all supplied
RGB stock components and acquires the stock borrow before adopting any state.
Incomplete stock is refused. Legacy snapshots with all stock components absent
retain compatibility. Existing exact pending-preservation guards still run first.
Every fallible validation completes before the synchronous in-memory commit;
no await, database write or callback occurs during adoption. A borrowed stock
returns a structured error instead of panicking after partial mutation.

Product inventory: `src/wallet/offline.rs`, staged restore and one four-case
regression (invalid BDK, corrupt stock, partial stock, borrowed stock). Matrix
compares the complete serialized before/after wallet, including local sequence,
pending intent, BDK changeset, RGB database, artifacts, stock and address indices.
It deliberately supplies a higher sequence, new pending record, changed database
counter and address index. Existing successful restore/accounting tests remain.
This document is evidence only. No dependency, schema, crypto or artifact change.

## Actual validation

Installed Rust1.97.1, locked/offline:
- Baseline affected regression: FAIL1/1, exit101.
- Initial corrective full suite: FAIL129/131 (exit101). An extra stock-is-None
  refusal rejected existing fresh in-memory restoration fixtures. Removed that
  unneeded compatibility change; existing tests were not weakened or edited.
- Final `cargo test --lib --locked --offline --quiet`: PASS131/131, exit0,
  including the final four-case matrix and existing accounting/backup tests.
- `cargo check --target wasm32-unknown-unknown --locked --offline`: PASS, exit0.
  Type/build check only; no artifact rebuild or consumer publication.
- `cargo fmt --all -- --check` and `git diff --check`: PASS.
- Installed stable Rust1.98.1 `cargo clippy --lib --locked --offline -- -D warnings`:
  PASS, exit0. No installation or toolchain/settings changes.

Native production restore with synthetic records only. No IndexedDB, AEAD/CAS,
engine-runtime checkout lifecycle, browser, signing, broadcast, abrupt OS kill or
financial recovery evidence. The stock borrow refusal does not establish that
all controller/Lightning writers share one queue; caller lifecycle guards remain
necessary. Strict existing-only restore still owns complete-snapshot requirements.
The ordinary legacy restore API does not authenticate a caller's snapshot.

## Compatibility, evidence reuse and rollback

Wallet PR39 remains 8028b45f7cdfdefd1dcdc7553d33e0fa10cc79dc; protocol1.9.0 and
all native/signer/security repair pins unchanged. This is a source descendant,
not a silently adopted tip. Prior driver/security/restore evidence keeps its
original revision and level; the new change affects malformed/busy active restore.
Exact successful restoration and legacy all-absent stock remain supported.
Pending preservation does not authorize lifecycle transitions or spending.

SAME active-default authenticated restore/flush/CAS, guarded confirm/apply and
serialized served bilateral execution remain OPEN. Seller per-UTXO spent/change,
actual R1/R4 and required live reverse remain OPEN; R3 UNAVAILABLE; THE-945 held.
THE-872 operator hold retained. No isolated watch-only writer attachment, JS
authority journal, funding/mining, serving, repin, merge or release. Finite
financial envelope unused. Removed native review policy remains removed;
Chief reviews comments and Software retains source execution ownership.
Rollback: revert this source commit before consumer adoption, restoring unsafe
partial-mutation semantics. No durable state or asset identity was migrated.
