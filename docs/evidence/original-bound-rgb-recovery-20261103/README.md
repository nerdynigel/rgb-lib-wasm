# Original-bound standard RGB self-recovery candidate

Implemented the missing wallet-owned REGTEST recovery for an expired signed
ordinary RGB original, including a recorded Bitcoin broadcast attempt and two
RGB-bearing inputs without an assignment-free fee input. This creates a NEW
standard RGB self-transfer and retains the old request, signed bytes and holds.

The explicit prepare_wallet_rgb_recovery entry point binds the original operation,
transaction and complete signed-byte SHA256. It verifies unchanged actual Stock,
all original RGB allocations, a recorded wallet-owned witness receive, bounded
fees and exclusively owned outputs. Existing BDK/PSBT/RGB colouring/external-send
lifecycle is reused. Ordinary prepare rejects the recovery marker. Exact source
reservations are omitted only from a temporary qualification view; no DB row or
Stock is deleted or reconstructed. Every unrelated native/caller hold remains.
A third competing hold refuses; one original cannot authorize competing recoveries.

Sign/submission use existing durable one-use attempts and original retention.
They independently recheck the old transaction is not observed. Only an exact
independently confirmed conflicting Bitcoin transaction permits existing conflict
reconciliation, before recovery Stock finalization. Mempool/absence/expiry never
releases the old operation. Pending-file restore remains observation-only.
The optional request marker makes older strict request parsers fail closed;
ordinary requests without it keep their previous serialized bytes.

Validation: 180 native library tests PASS (eight new authorization/Stock/BDK/DB
regressions), wasm32 library check PASS, Clippy library with warnings denied PASS,
formatter and diff check PASS. The native graph tests use real BDK signatures and
standard NIA genesis Stock on invented unfunded coins. They do not supply a valid
outgoing consignment or establish financial recovery, standard live settlement,
or Lab acceptance. Browser/SDK integration, complete v10 recovery, guarded release
and actual held d98ece81 recovery remain REQUIRED, not delivered by this commit.

Initial compilation found ambiguous hash-trait qualification and was corrected.
An initial host Clippy --tests invocation selected wasm-only integration tests
and established single-thread Arc test fixtures; that invocation failed and is
retained by hash, not called passing. The relevant host library strict lint,
native library regressions and wasm32 compilation are separately recorded.

No dependency versions changed. No live signature, delivery, financial broadcast,
mining, reset, original replay or hold release occurred in this source checkpoint.
Main remains untouched. Full BTCX delivery status stays in the integration branch
PROGRESS.md / PR252; this is the Native candidate component only.
