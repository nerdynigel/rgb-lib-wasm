# Private exact incoming application — 15 October 2026

PARTIAL / NOT SAFE TO ACCEPT. Predecessor f414cec8fe598bfba0ff387c48da8adef7fe8b13.

Inventory: ADD src/wallet/incoming_application.rs and this note; MODIFY
src/wallet/{mod,incoming_bdk,incoming_observation,incoming_post}.rs. No dependency,
lock, schema, feature, signer, asset, protocol or runtime-pin change.

FrozenIncoming now retains the exact established BDK Update, witness transaction,
recipient/receipt, complete source, valid transfer/resolver and complete prospective
post, bound by Weak/Rc identity to the original RGB engine. PreparedIncoming is a
private non-Clone/non-serializable value holding an exclusive mutable reference to
that SAME Wallet. Regtest, consumer-owned persistence and complete source equality
are required. There is NO public Rust/WASM consumer, durable-intent acknowledgement,
signing permission or production path added by this change.

The consuming synchronous core rederives precise DB changes before mutation, refuses
shared DB ownership, applies established Stock::accept_transfer and the exact BDK
Update, persists BDK to MemoryStore, installs the derived DB rows into the existing
DB owner, removes the exact consignment, and compares the COMPLETE resulting
WalletSnapshot to the prospective checkpoint without masks. It does not adopt a
projected Stock or BDK wallet. The capability is consumed by value; dropping it before
application preserves state. A failure after mutation is ambiguous and MUST remain
quarantined by the embedding journal; this core neither rolls back nor authorizes
retry. Holding &mut Wallet prevents ordinary interleaved wallet mutation, but is not
an authenticated host/session/expiry/debit proof.

The existing actual NIA ContractBuilder/export/validate/Stock projection fixture is
shared by two tests: the prior independent primitive comparison remains intact;
the new private consumer matches the complete post. It refuses wrong-engine,
stale-source, legacy persistence and shared-DB cases unchanged, proves preconsume
drop unchanged, and rejects the completed exact receipt using ORIGINAL consignment
bytes. Graph/inclusion/prevouts are SYNTHETIC, no signing/broadcast/live financial
operation. This is in-memory engine proof, not IndexedDB/journal durability.

Validation uses existing toolchain 1.97.1-x86_64-unknown-linux-gnu and process-only
PATH=/home/nigel/.cargo/bin:$PATH, CARGO_HOME=/home/nigel/.cargo,
RUSTUP_HOME=/home/nigel/.rustup:
- cargo test --offline --locked --lib incoming_post: PASS6/6 before final shared-DB assertion.
- cargo clippy --offline --locked --lib -- -D warnings: PASS after exclusive DB correction.
- scoped rustfmt --edition2024 --config skip_children=true --check; git diff --check: PASS.
- cargo test --offline --locked --lib: PASS167/167 (31.95s), final source including shared-DB assertion.
Initial compilation rejected cloning non-Clone WalletSnapshot; tests use its existing
serde round-trip instead. Missing explicit Rc import corrected. Initial Clippy
arc_with_non_send_sync corrected by preserving the existing unique Arc DB owner via
Arc::get_mut before mutation; no lint suppression or type/architecture change.

NEXT: private durable intent/debit-first host composition, current session/expiry/
revocation checks, complete post journal CAS/read-back and cancellation/error
quarantine. Do not expose or promote this core before those are proven. Async
configured-indexer observation remains unproven. Coherent immutable facade/lock/wallet
source integration follows the grouped engine consumer; Chief retains grouped
SDK/default+TEST/strict SDK Clippy/WASM/ABI/native-IDB/browser and independent
acceptance. Existing native --tests WASM-only imports and SDK ClippyFAIL40 unresolved.
Runtime wallet8c08a0ba/facadec7c04c07/enginea7210b82/protocol1.9.0 unchanged.
THE-872 source hold and THE-945 source+readiness prerequisites retained. Original
construction/live/SAME-case seller/buyer/fee BTC+RGB accounting/items5/16/18/19/R1/R4/
immediate-kill/required live reverse remain open; R3 unavailable. No heavy/build/
browser/financial replay, artifact adoption, source-completion verdict, release,
APPROVE or merge. Rollback source only, preserving journals and consumed cases.
