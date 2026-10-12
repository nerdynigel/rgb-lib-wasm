# Memory-only collaborative engine prerequisite — 12 October 2026 UTC

Plan37 and Chief comment013a95ce applied. Original acceptance OPEN / NOT SAFE TO ACCEPT.
Sole Software writer; supported issue workspace switch confirmed registered engine
f85beea3-1dd0-403d-aa2d-9f495e2cfef0, branch feat/the-876-transfer-export,
clean base e6fc0356c0e9676adaed870ad064ca3e0ef1e494 before edits. Authorised
additive isolated source work only. Tested source f508554468960774c2427d6f0a7aa388150d0437.
Final publication adds only these evidence files; source hashes in the safe receipt.

## Change and persistence audit

`color_psbt_and_consume_in_memory` reuses the original coloring/consume/transfer
implementation through a private shared core. It refuses empty asset requests
and unavailable/busy stock, colors a cloned PSBT and replaces caller PSBT only
on success. No flush, backup, signing or broadcast. Stock can be partially
mutated on a later engine error: callers MUST discard/poison their isolated
handle rather than retry. This API does not create consent or signing authority.
The ordinary async method still invokes the same core then legacy flush, with
its original error/PSBT mutation semantics preserved.

Audit also found `register_collaborative_outgoing_transfer` ends in
`update_backup_info(false)` plus `trigger_auto_backup`. Added
`register_collaborative_outgoing_transfer_in_memory` reuses ALL its original
contract/outpoint/allocation/arithmetic/replay checks and real database outgoing
rows. Ordinary method calls it and retains backup on newly registered rows;
exact replay still has no backup. Memory-only path does not schedule persistence.
The shared `_save_transfers` and `memory_db` setters modify only in-memory rows;
no IDB, flush or backup inside that called path. This is the existing aggregated
outgoing row model, not a new synthetic ledger or recipient-consent validator.
`get_address` remains excluded: its backup is NOT changed or used in this slice.
No default behavior, dependency, crypto, signer or shared protocol change.

## Executed criterion evidence

| Requirement | Evidence and practical limit |
| --- | --- |
| Remove implicit persistence from isolated consumption | Source split and compiled shared core; no successful funded consume executed |
| Remove auto-backup from isolated real row registration | Source split and called-path audit; no successful funded row mutation executed |
| New seams refuse invalid state without writes | Actual fresh retained Firefox test1 PASS: empty asset map, nonempty unknown contract reaching coloring/stock lookup, empty registration and unknown asset registration |
| Caller PSBT/state/storage unchanged after tested refusals | Same actual test: PSBT equality, complete engine snapshot equality excluding explicit sequence advance, raw IDB sentinel equality, schema equality and observed reads0/writes0 including delayed backup turn |
| Exact operation/association/consent/all recipients; encrypted durable mutation | OPEN; existing facade adoption exposes no mutation. Software owns remaining integration |
| Financial original-put failure/immediate crash/recovery/coherent balances | OPEN; no financial recipe executable or released here |
| Wallet actual affected release artifact | OPEN; facade/wallet pins and artifacts unchanged |

Final test artifact and source SHA256 are in THE876-MEMORY-ONLY-ENGINE-receipt.json.
Earlier fresh subset test1 PASS on a separate retained profile; then added
nonempty-unknown-contract core refusal and reran affected test on NEW profile,
1 PASS/0 FAIL/2 filtered,1.92s. No failed test in this slice. Profiles remain
under ignored `.state/the876-memory-only/`; no profile contents or secrets published.
Native IndexedDB is real; instrumentation counts attempted transactions and
refuses writes. This negative result is NOT durable financial accounting proof.
No old/preserved profile, fixture, funding, issuance, block, signing or broadcast.

Commands (run-owned target/logs; all exit0):
```sh
/home/nigel/.local/bin/cargo test --locked --offline --target wasm32-unknown-unknown --target-dir "$PAPERCLIP_RUN_SCRATCH_DIR/engine-target" --test existing_snapshot_wasm --no-run
/home/nigel/.cache/.wasm-pack/wasm-bindgen-8a74166b323833fc/wasm-bindgen-test-runner <run-target>/wasm32-unknown-unknown/debug/deps/existing_snapshot_wasm-024f3a2beba96ecc.wasm memory_only_pending_refusals_real_idb --exact --nocapture
/home/nigel/.local/bin/cargo check --locked --offline --target wasm32-unknown-unknown --target-dir <same> --workspace
/home/nigel/.local/bin/cargo clippy --locked --offline --target wasm32-unknown-unknown --target-dir <same> --workspace -- -D warnings -A clippy::field_reassign_with_default
/home/nigel/.local/bin/rustfmt --edition 2024 --check src/wallet/rust_only.rs src/wallet/online.rs tests/existing_snapshot_wasm.rs
git diff --check
```
Clippy allows only the previously documented baseline lint; strict Clippy is
NOT claimed run. Initial test compilation1m55s; workspace check56.14s and
scoped Clippy4.70s. Compiler/source lock unchanged. Installed runner Firefox
with GECKODRIVER=/usr/local/bin/geckodriver, MOZ_HEADLESS=1,
WASM_BINDGEN_TEST_ONLY_WEB=1, NO_PROXY/no_proxy localhost,127.0.0.1;
capability JSON points at the NEW retained profile. No host installation.

## Remaining owner/sequence and rollback

Software owns the incomplete plan37 outcome: immutable consent + association
+ exact colored transaction/transition/all-recipient validation before memory
mutation; real outgoing rows; poisoned handle on any error; explicit complete
secure_store encrypted CAS only after full validation. Publish engine FIRST,
then isolated facade repin/integration, wallet entry/pin/full artifacts and
actual affected UNFUNDED browser evidence. Do not expose a raw unbound mutate
method as a shortcut. Wallet retail journal pending intent/consent is an existing
source harness only; it must stay held until that real path and financial proof.

After source readiness submit ONE executable immutable finite fresh recipe to
Chief BEFORE financial launch, covering exact profiles/source/artifacts/tip,
original native put failure and abrupt-process interruption/restart, coherent
same-case balances and limited effects. No recipe submitted/released here.
This is not an authority blocker or acceptance waiver. Chief reconciles this
bounded engine prerequisite and routes SAME-task serial Software continuation.
All consumed-case/downstream/freeze/merge holds retained.

Rollback: revert these additive seams before downstream adoption; if adopted,
restore prior candidate pins/capability without deleting or replaying any records.
No financial or persistent source snapshot change requires cleanup.

Compatibility:
- btcx-wallet-protocol:1.9.0 @2a849f20a1d891748454f13e3d45b0d2a84c9c1b unchanged
- btcx:wallet pin dd4c12b846da9a1e1956766a35296893702102a4 unchanged
- btcx-wallet:f70b4da59aee933d50bc3306fcc7ac2d604de05a unchanged
- RLN:a918c8943ab012782750da1fea3d82e57ba15078 source candidate; wallet2a4d5edf unchanged; engine publication in issue
- signer:074de663746b654bbd1cb9756e34c531101edec8 unchanged
- rust-lightning:0939f7751fb876e02caec9cd0a778794b81eee9e unchanged
- breaking protocol change:NO
- peer repo update required:YES remaining isolated facade/wallet integration
