# Isolated producer engine prerequisite — 12 October 2026

Plan39 applied. PARTIAL; original financial acceptance OPEN / NOT SAFE TO ACCEPT.
Registered engine workspace f85beea3; clean base6aabb65a67be217942715d59ba44e6a933b43449.
Tested source591c6f659e2f5ce8f1c1f0262c7779c69a502658. No financial recipe released.

## Persistence audit and change

Added `reveal_funding_address_in_memory(colored)` using established BDK address
revelation and `MemoryStore::persist`, which merges only an in-memory changeset.
External is the colored keychain; Internal is vanilla. Commit the complete
encoded snapshot before funding any returned address. No signing or secret export.

NIA issuance calls `update_backup_info` and `trigger_auto_backup` on success.
Split out `issue_asset_nia_in_memory` using the SAME existing contract builder,
stock import, allocation selection, issue rows and asset details. Ordinary
`issue_asset_nia` retains its backup/update/completion logging after core success.
Late failures can partially mutate state: isolated caller must poison/discard,
never retry. This is not a synthetic ledger or a new issuance algorithm.

`go_online` sets the indexer/online handle only; `sync`/`sync_db_txos` obtain an
indexer update, apply it to BDK, merge `MemoryStore`, and reconcile real TXOs in
`InMemoryDb`. Neither called path invokes wallet IDB, flush, backup or dispatch.
`add_asset_to_db`, `_save_transfers` and database setters are in-memory. No need
to change these methods. Do not call `get_address`, ordinary issuance, ordinary
bootstrap/restore, `create_utxos_end`, refresh/send/broadcast or plaintext flush
from the isolated producer. Default behavior/dependencies/signing unchanged.

## Executed evidence and limits

NEW retained Firefox profile297203fd-1f4f-4003-b40c-54c431cf382e:
`memory_only_producer_real_idb` PASS1/0failed/3filtered,0.99s. Real native IDB
sentinel bytes/schema unchanged, reads0/writes0 through delayed backup turn.
Successful colored+vanilla address revelation and snapshot restoration produce
the same next addresses. Empty issuance and nonempty insufficient-funds issuance
refuse; complete engine state equals prior state apart from explicit snapshot
sequence advance. No synthetic/funded fixture, no issuance success, network,
funding, blocks, signing, broadcast or preserved profiles.

Commands (ordinary user, run-owned target):
- cargo test --locked --offline --target wasm32-unknown-unknown --target-dir <run>/engine-target --test existing_snapshot_wasm --no-run: PASS.
- installed wasm-bindgen-test-runner0.2.117 <run-target>/existing_snapshot_wasm-024f3a2beba96ecc.wasm memory_only_producer_real_idb --exact --nocapture: PASS.
- rustfmt --edition2024 --check src/wallet/offline.rs tests/existing_snapshot_wasm.rs and git diff --check: PASS.
Additional checks and hashes in receipt. No full suite, positive financial issuance,
consumption/outgoing accounting, native financial-put failure, abrupt crash or
wallet release claim.

## Residual and rollback

Software owns secure-session regtest-only producer wrapper/caller/harness,
explicit encrypted insert/CAS, poison on errors; upstream-first facade and wallet
pin/artifact/affected proof. Then ONE executable immutable finite NEW financial
recipe to Chief BEFORE launch, including exact source/artifacts/origins/identity/
tip/effect budgets/barriers/same-case balances and failure/crash/reconciliation.
No current financial release; all downstream/merge/consumed/preserved holds retained.
Rollback additive seams only (prior engine6aabb65a), restore consumer candidate
pins together when adopted; retain snapshots/profiles and never replay records.

Compatibility:
- btcx-wallet-protocol:1.9.0 @2a849f20a1d891748454f13e3d45b0d2a84c9c1b unchanged
- btcx:dd4c12b846da9a1e1956766a35296893702102a4 unchanged
- btcx-wallet:364c3d47d81c345899101caf42a6477fcd107b14 unchanged
- RLN:900f92b7fe7994b0710180b1d5f4bfa0ef327c75 unchanged; engine tested source above
- signer:074de663746b654bbd1cb9756e34c531101edec8 unchanged
- rust-lightning:0939f7751fb876e02caec9cd0a778794b81eee9e unchanged
- breaking protocol change:NO
- peer repo update required:YES isolated facade/wallet producer chain
