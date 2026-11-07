# Registered preparation early-failure recovery — Native candidate

Native-only implementation checkpoint. Mission ACTIVE / NOT DELIVERED. This
candidate is not connected to the secure SDK or customer wallet yet, and is not
installed on Lab. Wallet8fb5bf9 /SDK468a98d /Native1f01a53 remains the last built
wallet cohort. Lab f421d175 /Wallet939b2ca2 and main were not changed.

The engine now persists an exact bounded preparation registration before online
validation, synchronization or input selection. Registration is metadata only:
it creates no recipient, selects no input, changes no balance and invokes no
signer/broadcaster. It is exact-idempotent and never renews original expiry.
The secure facade/host integration must receive this durable registration ACK
before committing the host financial preparing barrier or dispatching prepare.
A missing registration ACK does not authorize selection. Retain the host request
identity for metadata-only retry/recovery; never substitute a new budget.

New abort_bitcoin_input_preparation requires that same registered full budget,
actual REGTEST wallet configuration, no Native original and an unrestored pending
registration. It commits an append-only aborted receipt before ACK. Unknown IDs,
getter absence, generic errors, expiry, changed terms and restored pending state
are never negative evidence. Any original, even a cancelled unsigned original,
refuses this early-abort path: use existing exact-PSBT cancellation/recovery.
After a failed flush the caller retains its barrier; an existing aborted receipt
can be durably acknowledged again without new financial authority. No signing,
broadcast, input release, fee collection or settlement primitive was changed.
Older unregistered unknown host barriers remain unresolved; this API does not
retroactively classify them from absence.

A dedicated bounded map retains at most128 wallet-owned registrations/receipts,
without invented transaction IDs, outpoints, balances or Stock. No automatic
pruning/eviction. Snapshot version6 and encrypted-backup version8 preserve it and
force older engines to refuse rather than ignore it. Legacy empty-map snapshots
and backups through7 remain readable. Exact budget/ID/canonical ordering and
original relationships are validated on snapshot encode/decode, backup decode/
export and installation. Snapshot CAS rejects lost/changed registrations,
terminal rollback, restored-fence rollback, stale writes and stale-tab abort
which omits an already committed original. Existing signing/broadcast/original
append-only checks remain. File restore fences every registration and existing
original; even registration-only target engines cannot be overwritten by import.
Backup comparison permits only the existing conservative restore-flag promotion.

Validation: 253full library cases PASS, zero failed/ignored, in224.76seconds; the
preliminary251-case library suite also passed.
Nine new cases cover local real BDK selection failure, negative receipts,
idempotency/unchanged expiry, originals refusing early abort/rebuild, restored
fences, capacity/malformed requests, retained-state validation, version6/CAS,
registration-only pending/terminal complete-state restore and encrypted version8
original-plus-registration roundtrip/comparison. Invented local coins, no
network/faucet/miner/broadcast: these are component safety checks, NOT funded
Native preparation, wallet-device acceptance or atomic settlement.

Strict production-library Clippy, formatting and locked wasm32 compile pass.
The broader --lib --tests Clippy command fails on existing native test Arc use,
untouched wasm-test imports and an existing rust_only test-module placement.
Its raw failure is retained alongside a precise source comparison to1f01a53;
the parent broad command was not executed and no lint was disabled or waived.
Initial compile failure for a missing public re-export was corrected and retained.
No dependency or lockfile changed. Four-thread final test scheduling bounds
concurrent local KDF pressure; no production clock, KDF or assertion was weakened.

Next: exact-pin the secure SDK, add bounded secure registration/abort facades,
connect typed host outcomes and compatible persistence/backup readers, finish RGB
preparation and deliberate cohort admission/build, then guarded REGTEST deployment
and real funded preparation/recipient spend-back/three-maker/both-way partial and
adverse recovery acceptance. Product/admin/FigCraft/independent gates remain open.
No financial action or Lab deployment occurred in this checkpoint.

Compatibility: shared protocol621f2c17 (optional1.24), BTCX installedf421d175,
Wallet candidate8fb5bf9/installed939b2ca2, RLN468a98d unchanged, signer and Lightning
pins unchanged. Market protocol breaking change:NO. Native persistence breaking
reader change:YES, snapshot6/backup8 when applicable. Peer update required:YES;
this implementation must be deliberately integrated, built and tested before use.
