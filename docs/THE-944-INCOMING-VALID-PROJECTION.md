# Valid NIA incoming projection proof — 15 October 2026

PARTIAL / NOT SAFE TO ACCEPT. Source predecessor c5a286b43c1d7a9d3269f1007e1a85ab4a077c9e.
Inventory: MODIFY src/wallet/incoming_post.rs (cfg(test) only); ADD this note.
No product code, feature, dependency, lock, schema, signer, asset or runtime-pin delta.

New actual-module test `valid_nia_transfer_projects_full_checkpoint_without_active_mutation`
issues a clearly fake 1107-unit NIA through established ContractBuilder, imports it into
sender Stock, exports an actual RGB consignment through color_psbt_and_export, and
validates it using the pinned RGB validator and a frozen PreFetchResolver. The
transaction graph, inclusion and prevouts are SYNTHETIC; no chain/indexer, financial
operation, signing or broadcast occurs. No runtime fixture export is added.

The receiving wallet passes the actual local exact-receipt selector with canonical
recipient/script, ContractId, canonical consignment bytes, DB relationships and units.
The complete detached Stock+BDK+DB projection succeeds; wrong units refuse with the
complete source snapshot AND staged BDK unchanged. All active state remains unchanged
on successful projection. Exact received Stock allocation is 1107 units.

Then the disposable in-memory receiving wallet independently applies the established
BDK Update and Stock::accept_transfer plus explicit receipt/status/pending-script
updates. It does NOT adopt the projected snapshot or call project_db to generate the
actual result. Its COMPLETE serialized WalletSnapshot equals the prospective result:
Stock stash/state/index, complete BDK graph, DB rows/counters, sequence, pending intent,
artifacts, consignments, timestamps and reuse metadata. No comparison masks.
BDK persist here targets MemoryStore only, not IndexedDB or a financial journal.

Initial compile E0308: expected RevealedValue, not Amount in test assertion; corrected
the assertion to inspect actual revealed units. Initial validator failure InvalidProofType:
test OP_RETURN followed P2TR outputs. Fixture corrected to OpretFirst ordering (OP_RETURN
first, receipt vout1), retaining the validator and all safety assertions. No product
change made to bypass this failure.

Executed with process PATH=/home/nigel/.cargo/bin:$PATH, CARGO_HOME=/home/nigel/.cargo,
RUSTUP_HOME=/home/nigel/.rustup, RUSTUP_TOOLCHAIN=1.97.1-x86_64-unknown-linux-gnu:
- cargo test --offline --locked --lib incoming_post: PASS5/5 (161 filtered).
- cargo clippy --offline --locked --lib -- -D warnings: PASS.
- scoped rustfmt --edition2024 --config skip_children=true --check and git diff --check: PASS.
- cargo test --offline --locked --lib: PASS166/166 (31.43s), final source.

This closes only the prior empty-Stock gap for the synchronous local projection and
complete in-memory prospective/actual comparison. It does NOT prove the async indexer
observer, opaque SAME-engine one-shot consumer, durable intent/debit acknowledgement,
full post-root journal CAS/read-back, served acknowledgement, recovery or signing.
No browser/IndexedDB, immediate process-kill, live settlement/accounting, R1/R4/reverse
or independent acceptance uplift. Historical native --tests WASM-only imports and SDK
ClippyFAIL40 remain unresolved. R3 unavailable without approved policy authority.

NEXT SAME owner: bind FrozenIncoming to one opaque SAME-engine preparation and retain
exclusive engine custody; consume once only after durable intent/debit acknowledgement;
apply established Stock/BDK/exact DB transition, compare complete actual post and commit
full post-root with exact CAS/read-back. Preserve original source, obligations and
ambiguous recovery refusal. Keep incoming/default public fences until consuming
integration is proven. Then grouped coherent facade/lock/wallet source pin and finite
affected-check manifest for Chief's existing Reliability/independent acceptance chain.
No heavy/build/browser/financial replay, runtime adoption, APPROVE, merge or release.
Runtime wallet8c08a0ba / facadec7c04c07 / enginea7210b82 / protocol1.9.0 unchanged.
THE-872 source hold and THE-945 source+readiness prerequisites retained. Rollback this
test/evidence commit only; preserve original financial state and consumed cases.
