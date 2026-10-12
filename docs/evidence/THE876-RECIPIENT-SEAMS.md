# Plan42 recipient engine seams — 12 October 2026

PARTIAL / original acceptance OPEN / NOT SAFE TO ACCEPT. Sole Software writer;
registered f85beea3 confirmed at baseffc3ab1d. Tested source 3536e8a6d0f4e350fb18f909ce9d26b50609bc59.
No financial recipe released, no wallet/facade/Lightning pin changes.

Ordinary witness_receive still invokes the same core, then update_backup_info and
trigger_auto_backup. Additive witness_receive_in_memory retains established BDK
address derivation, invoice, pending script and batch/asset/recipient rows without
snapshot/backup. BDK persist merges MemoryStore only.

Direct import_witness_consignment_in_memory uses the extracted ordinary
_wait_consignment_async core, selected by real stored witness recipient/batch.
Requires WaitingCounterparty/incoming/unbound witness. It uses established
consignment decode/validation, witness txid/vout/script checks, extracted real RGB
assignments, asset import and DB asset/transfer/coloring rows; moves to
WaitingConfirmations. It rejects IFA/media-fetch paths and suppresses ACK/NACK;
invalid consignments return errors instead of network side effects. Valid contract
import errors propagate on the isolated path; ordinary expect behavior retained.
No stock acceptance until established confirmation path. Errors can partially
mutate memory; facade must poison/discard and never persist/retry failed state.

settle_witness_receives_in_memory checks online and only incoming NIA witness
WaitingConfirmations rows. Calls established _wait_confirmations_async with
sync enabled: actual indexer confirmations, memory-only BDK/TXO sync, stock accept,
pending witness clearing and Settled rows. No outgoing ACK/broadcast or ordinary
refresh dispatch. No implicit snapshot/backup in traced call graph.

Correction to prior audit: online.rs:1170 and1857 backup calls belong to outgoing
registration/create_utxos_end; they are not in the incoming confirmation chain.
The direct path excludes proxy/media/reject-list and ordinary accept_transfer's
flush. Existing ordinary receive/proxy/refresh signatures remain unchanged.
No custom crypto, signing, synthetic rows or caller-declared settlement.

Actual NEW retained UNFUNDED Firefox case 19dc4dc3-f077-4e94-9074-c0dc8fd1445e:
memory_only_recipient_real_idb PASS1/0failed/4filtered,0.89s. Established receive
rows restored into a distinct engine; next receive identity distinct; missing
recipient/malformed base64/malformed serialized consignment refuse on separately
restored discarded handles. Real native IDB sentinel bytes/schema unchanged;
delayed reads0/writes0. Earlier test passed but was tightened to discard on each
failure. Two compile errors (borrowed core args, WalletSnapshot non-Clone) fixed
before final compile/test. Profiles retained; no profile bytes or secrets published.

Commands: cargo test --locked --offline --target wasm32-unknown-unknown --test
existing_snapshot_wasm --no-run PASS; installed wasm-bindgen-test-runner0.2.117
<debug wasm> memory_only_recipient_real_idb --exact --nocapture PASS; cargo check
--locked --offline --target wasm32-unknown-unknown PASS; rustfmt --edition2024
--check <three changed Rust files> and git diff --check PASS. Scoped Clippy passed
before final error-propagation/recipient restriction refinements; final check/test
cover final source. Exact command/case/source/debug-WASM hashes in receipt.

OPEN: successful financial import/settlement, authenticated facade CAS and poison,
full wallet caller/artifact adoption, ONE immutable finite executable financial
recipe to Chief BEFORE launch, durable consent/native financial-put failure/
abrupt crash/coherent same-case balances; all original served/directions/cohorts/
admin/repaired-closure/independent requirements. No funding/issuance/sync/blocks/
signing/broadcast or crash executed. Software retains missing implementation proof.
Rollback additive engine source as a unit toffc3ab1d; preserve profiles/snapshots,
never reset/replay. Chief serial reconciliation via same issue, no new task/card.

Compatibility:
- btcx-wallet-protocol:1.9.0 @2a849f20a1d891748454f13e3d45b0d2a84c9c1b unchanged
- btcx:dd4c12b846da9a1e1956766a35296893702102a4 unchanged
- btcx-wallet:ad01c0bc80ac24d7ab2fe080dfb4d38bccd89a6f unchanged
- RLN:c518ec0ca94cc2db9f7e238001e4babed1e87a71 unchanged; engine tested source above
- signer:074de663746b654bbd1cb9756e34c531101edec8 unchanged
- rust-lightning:0939f7751fb876e02caec9cd0a778794b81eee9e unchanged; repair pending
- breaking protocol change:NO
- peer repo update required:YES facade/wallet/repaired closure adoption
