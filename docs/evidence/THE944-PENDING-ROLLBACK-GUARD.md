# THE-944 active pending snapshot rollback — 13 Oct 2026

Partial necessary source correction, existing feat/the-876-transfer-export / draft
PR3. Descendant of Chief-selected f885c3fe, d252a7a and published d74e014.
Source integration only; consumer b377de6 and RLN f4332a1c unchanged.

`restore_from_snapshot` preserved an exact pending record but accepted older
financial state, keeping only the higher local sequence counter. Native regression
`active_pending_restore_refuses_older_financial_state` FAIL1/1 (exit101) on d74e014:
a sequence1 snapshot carrying the same Consented intent was accepted over local
sequence101, with changed address derivation state. This is a synthetic executed
rollback, not observed fund loss. The correction refuses the older snapshot before
BDK/RGB decoding or mutation while an active pending record exists. Both Consented
and Applied are covered. Full before/after serialized state comparison passes;
only the test's subsequent snapshot increments its sequence.

Product inventory: src/wallet/offline.rs, local rollback guard and regression.
This document is evidence. No dependency, schema, crypto or artifact change.
Legacy backup restores without an active pending record retain their semantics;
exact current pending restores remain supported by existing tests. A failed flush
may advance the local sequence: an older restore is deliberately refused, rather
than guessing whether it is safe. Caller must reconcile ambiguous state.

Actual commands (installed tools; no installation):
- cargo +1.97.1 test --lib --locked --offline active_pending_restore_refuses_older_financial_state -- --nocapture: baseline FAIL1/1, exit101.
- cargo +1.97.1 test --lib --locked --offline --quiet: corrected PASS132/132, exit0.
- cargo +1.97.1 check --target wasm32-unknown-unknown --locked --offline --quiet: PASS.
- cargo +stable clippy --lib --locked --offline -- -D warnings: PASS (installed1.98.1).
- cargo +1.97.1 fmt --all -- --check; git diff --check: PASS.

Native synthetic-state evidence only. This guard cannot authenticate a snapshot,
prevent a forged higher sequence, or replace encrypted consumer CAS. Equal-sequence
imports still require consumer authentication/exact-record guards. No authority
transition, browser/IDB, signing or financial recovery proof. Prior driver, native,
signer, protocol1.9.0 and Lightning repair evidence keeps its original revision
and level. New check covers active pending restore rollback only.

Inspection of the saved RLN facade confirms secureWalletFlush still delegates to
the default wallet's flush, while accounting_snapshot owns a separate watch-only
engine and encrypted encoded CAS. Neither is attached/adopted here. The necessary
next integration is a SAME-active-default authenticated persistence lifecycle;
keep exact pending validation, independent consumer authentication, serialized
checkout, stale/failed writer refusal and signing holds. Do not use a JS authority
journal or concurrent writer.

OPEN: active-default authenticated restore/flush/CAS and guarded confirm/apply,
serialized bilateral served execution, seller per-UTXO attribution, R1/R4 and
mandatory live reverse. R3 unavailable; THE-945 held; THE-872 operator hold retained.
No financial envelope used, serving/funding/mining, rebuild/repin, merge or release.
Rollback: revert this source correction before consumer adoption; no migration.
Chief reviews by comments; no automatic per-slice source review stop.
