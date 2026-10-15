# THE-944 consumer-owned active restore — 13 Oct 2026

PARTIAL source integration, no candidate or financial acceptance. Existing
feat/the-876-transfer-export / draft PR3, parent
24402a7b32e6c1b134d90e2beb1c9bb9554c22ab. Chief comments d39fd62c and
4e20e13c applied in order: SAME-issue Software execution, comment review,
no native review stage. Selected f885c3fe ancestry, persistence handoff,
immutable consent and earlier atomic/rollback restore corrections preserved.

## Reproduction and grouped correction

Baseline targeted test FAIL1/1 (exit101): consumer-owned active restore accepted
an incoming snapshot without BDK state. That path could combine incoming RGB
rows with the handle's previous BDK state. This is native synthetic state evidence,
not an observed financial loss, browser restore or authenticated-storage attack.
The baseline stopped at missing-BDK; no executed baseline failure is claimed for
the other matrix cases.

Product: src/wallet/offline.rs only. Consumer-owned active restore now requires
complete BDK and all three RGB stock components, refuses detached/borrowed RGB
runtime and pending legacy saving, and refuses older sequences even before any
pending fill exists. Every refusal precedes state adoption. Existing exact pending
preservation remains; consumer ownership remains irreversible. Legacy restoration
without consumer ownership or active intent retains its previous compatibility.

Additive Rust-only restore_consumer_owned_snapshot constructs ONE strict complete
engine and hands persistence to its consumer before exposing the handle. It does
not attach the separate watch-only accounting writer, read IndexedDB, authenticate
ciphertext, acknowledge CAS, grant approval, consume RGB, sign or broadcast.
The embedding facade must authenticate/decrypt the exact stored record before
construction, retain that exact ciphertext for atomic CAS, serialize all runtime
users and poison a failed/ambiguous writer. Equal-sequence imports and higher-
sequence competing writers still need authenticated exact-ciphertext CAS; this
local rollback guard alone is insufficient.

## Actual validation

All final commands exit0 on existing installed tools; no installation/settings change:

- cargo +1.97.1 test --lib --locked --offline consumer_restore -- --nocapture: PASS2/2.
- cargo +1.97.1 test --lib --locked --offline --quiet: PASS141/141.
- cargo +1.97.1 check --target wasm32-unknown-unknown --locked --offline --quiet: PASS.
- cargo +stable clippy --lib --locked --offline -- -D warnings: PASS (installed1.98.1).
- cargo +1.97.1 fmt --all -- --check and git diff --check: PASS.

Four-case native refusal matrix: missing BDK, all RGB components missing, older
sequence without pending, detached runtime. Each asserts unchanged full financial
snapshot/sequence and retained ownership. Constructor test asserts exact pending
and financial-state preservation, no autosave scheduling/sequence increment, and
legacy persistence refusal. Interim140/140 predates the constructor test; final
141/141 is applicable. No test weakened; synthetic native evidence only.

## Remaining proof, compatibility and rollback

| Requirement | Exact evidence level / remaining Software work |
|---|---|
| Complete SAME-engine consumer restore and ownership | Native source predicates/serializer; no authentication inferred |
| Active-default encrypted restore/flush/exact-ciphertext CAS | OPEN: facade integration and actual IDB proof; saved isolated accounting writer is not equivalent |
| Current guarded review/confirm/apply, both approvals, own signatures | OPEN: installation/storage is not spending authority |
| Bilateral SERVED coordinator/bridge non-zero terms execution | OPEN: historical driver proof retains driver level |
| Seller per-UTXO spent/change and all-party same-case accounting | OPEN |
| R1 abrupt OS kill, R4 financial persistence/refresh, immediate crash, required live reverse | OPEN |
| R3 | UNAVAILABLE, unchanged |
| Frozen-candidate independent acceptance | THE-945 held; THE-872 operator hold retained |

No consumer/security pin or artifact adoption: engine b377de6, saved facade
f4332a1c, wallet8028b45 and protocol1.9.0 remain unchanged. Source descendant only,
not a released dependency. Previous security, driver and financial evidence remains
valid at its original revision/level. New source affects consumer restore integration;
required future IDB/active-default/financial proofs remain missing. No financial
envelope, rebuild of distributed artifacts, repin, serving/funding/mining, merge or
release. WASM check is type/build validation, not an artifact release.

Before adoption, rollback by reverting this source delta. No durable migration or
financial state change occurred. After future adoption, never erase ambiguous
intent or enable legacy writes to recover a failed CAS. Software continues SAME
issue; Chief owns persisted continuation reconciliation, with no per-slice review
stop or completion/QA promotion.
