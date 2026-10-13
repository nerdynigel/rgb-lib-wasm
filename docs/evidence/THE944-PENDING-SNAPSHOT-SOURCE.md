# THE-944 exact pending record preservation — 13 Oct 2026

Source candidate only. This closes the identified schema omission in source;
it does not close active-engine financial authority, authenticated CAS or recovery.
Existing draft PR3 and branch feat/the-876-transfer-export are reused.

## Baseline and inventory

The saved local HEAD f0397e431e5b0d151403e76df8a8100ad5a7e8c7 was an ancestor of
already published/pinned b377de6aec807adb597e7f0bced7fa578a54713b. Inspected the
three intervening commits (28462b2/cac8ac3/b377de6), independently read origin and
PR3 head, then fast-forwarded to exactly b377de6 before final validation. Those
existing fixes address witness invoice accounting and OP_RETURN output indexing;
they do not overlap this delta. No consumer pin or generated artifact changed.

Product: src/wallet/pending_fill.rs (typed immutable record and strict decoding),
idb_store.rs (optional schema field), offline.rs (fresh/ordinary/strict restore
and snapshot preservation), backup.rs (encrypted local/VSS payload preservation),
mod.rs (module). Tests live in pending_fill.rs/offline.rs/backup.rs. This document
is evidence. No infrastructure or shared protocol changes.

## Contract and safety

The existing facade record encoding (btcx_pending_fill_v1) is preserved: exact
request, inspected facts, colored PSBT, existing btcx-accounting-consent-v1 SHA256
digest and Consented/Applied state. Established Bitcoin hashes are reused.
Decode refuses changed digest/preimage, invalid enum, unknown record/request/facts
fields and normalization that changes the object. Old snapshots/backups without
this optional field remain readable. Expired records remain available for
reconciliation; execution expiry enforcement stays at the guarded use boundary.
The record has private fields and grants NO ownership, policy, consent, signing or
spending authority. Integrity checking does not validate a transfer's economics.
No new signing/apply API or watch-only attachment is introduced.

The source defect was established by the previous schema/restore audit. No live
loss was claimed or reproduced: browser artifacts remain held, and historical
financial cases must not be replayed. New native fixtures are explicitly synthetic
storage-only records, never valid RGB construction or approval evidence.

## Validation

From this repository, /home/nigel/.cargo/bin/cargo:

- test --lib pending_fill --locked --offline: PASS5/5 at published baseline + delta.
  Exact ordinary/strict restore and snapshot preserve the record; changed terms,
  digest, unknown shape/state refuse; encrypted production backup payload and
  restore preserve it. Native backup test excludes browser Date bookkeeping and
  holds automatic IndexedDB scheduling; it does NOT prove a browser commit.
- test --lib wallet::offline::address_reuse_tests --locked --offline: PASS14/14
  on the saved-source delta before baseline advance; superseded by final full lib
  result recorded in the BTCX containing evidence/PR manifest.
- check --target wasm32-unknown-unknown --locked --offline: PASS (type/build check;
  no generated WASM artifact, browser run or serving).
- fmt --all -- --check and git diff --check: PASS.
- clippy --lib --locked --offline -- -D warnings: FAIL3 existing diagnostics in
  unchanged online.rs (field_reassign_with_default at1333/1371; useless_conversion
  at2842). Not suppressed; no strict-lint pass claimed.

Initial test-only compile errors were corrected. The initial native public backup
ceremony test FAILED at js_sys::Date::now in update_backup_info. Split native
payload/encryption/restore coverage from browser-only bookkeeping; real browser
backup/IDB and abrupt-crash proof remain OPEN.

## Compatibility, reuse and rollback

- btcx-wallet-protocol: 2a849f20a1d891748454f13e3d45b0d2a84c9c1b (1.9.0).
- btcx: PR177; containing evidence commit names exact current head.
- btcx-wallet: 24c7a88d1fda7acc04f59e719c0b7badad3c93c5 (PR39 unchanged).
- RLN: f4332a1c56d1d16682996fb717ecfaf4b4400cd4 (consumer pin unchanged);
  saved facade4fb1e8f not adopted.
- signer: 074de663746b654bbd1cb9756e34c531101edec8.
- rust-lightning: 9636ee11c2373688375beb84c46dfa36bb458319.
- breaking protocol change: NO. Peer repo update required: YES, explicit engine
  candidate selection and eventual dependency/artifact integration; NOT performed.

Prior driver, security and wallet source proof retains its exact original level;
no financial proof is supplied by this change. Default secure facade confirm/apply,
serialized lifecycle, authenticated stale-writer CAS, bilateral application path,
seller per-UTXO attribution, R1/R4 and required live reverse remain OPEN. R3 stays
UNAVAILABLE. No financial envelope used or THE-945 promotion.

Revert this source commit to return to b377de6. No persisted migration occurred.
Do not downgrade a future activated wallet carrying this record to an engine that
silently drops it; preserve the full encrypted snapshot instead. This rollback
instruction covers source-only work, not future financial effects.
