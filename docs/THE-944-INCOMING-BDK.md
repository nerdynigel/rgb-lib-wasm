# Detached incoming BDK projection — 15 October 2026

PARTIAL / NOT SAFE TO ACCEPT. Authorized SAME-owner incoming consumer source;
predecessor 8ffc9f24c2b126b95e195f042836953c93fe1e1d. No consuming capability,
application, financial operation, runtime re-pin or distributed artifact adoption.

Inventory: ADD src/wallet/incoming_bdk.rs and this note; MODIFY
src/wallet/incoming_observation.rs and src/wallet/mod.rs. No Cargo/lock/dependency,
feature, schema, asset, protocol or signer change. Facade still uses a7210b82.

The read-only exact observation now derives a detached BDK changeset using the
persisted changeset PLUS the live stage. The detached BDK owner loads public
descriptors without extracting keys. Every accepted transaction input has an
exact fetched previous output; prior transaction txid/size and vout must match.
Known graph output bytes must agree. Owned inputs must exist as canonical unspent
local outputs. Duplicates, missing or unproven local inputs, an already observed
transaction, invalid checked monetary totals and a conflicting checkpoint refuse.
All owned outputs are included, including Internal BTC change and External RGB
carrier outputs. BDK receives only the exact transaction and confirmed inclusion,
with existing checkpoints preserved. Before/after canonical outputs must be
identical except for the exact inputs' spent bits and the newly owned outputs.
No comparison masks or broad sync. Unrelated eviction/conflict/ancestor-position
changes refuse. The full prospective BDK changeset stays internal to FrozenIncoming;
it is NOT a complete RGB/DB checkpoint and grants no application authority.

Observation bounds: 128 inputs, 128 outputs, 256 existing local checkpoints;
existing 128-history/8MB-consignment/4MB-transaction bounds retained. Every local
checkpoint must match the configured indexer's frozen chain, not extend beyond
its tip. Tip/genesis/status rereads remain. This retains configured-indexer trust,
not independent consensus/SPV verification. Prevout transaction identity uses the
established Bitcoin txid implementation; there is no second mutable network sync.

Executed with process-only PATH=/home/nigel/.cargo/bin:$PATH,
CARGO_HOME=/home/nigel/.cargo, RUSTUP_HOME=/home/nigel/.rustup,
RUSTUP_TOOLCHAIN=1.97.1-x86_64-unknown-linux-gnu:

- cargo test --offline --locked --lib: PASS161/161.
- cargo clippy --offline --locked --lib -- -D warnings: PASS.
- Changed-file rustfmt --edition 2024 --config skip_children=true --check and
  git diff --check: PASS.

Four new tests use actual Wallet/Stock/BDK and synthetic transaction graphs.
Positive projection proves a known BTC funding input, External carrier receipt,
Internal BTC change, unrelated output preservation, deterministic complete BDK
changeset and unchanged full active snapshot plus stage. Negatives cover missing,
wrong/duplicate/unproven local inputs, invalid totals, existing transaction,
checkpoint conflict and unconfirmed ancestor promotion. These are NOT real funded
settlement, RGB acceptance, async indexer or browser/IDB evidence.

Initial test run FAIL3/3: fixture incorrectly put a synthetic coinbase in the
mempool; fixed to a noncoinbase synthetic funding transaction. Next run FAIL1/3:
confirming its child promoted an unconfirmed ancestor and changed the unrelated
funding output's chain position. Strict equality was retained; positive fixture
now has confirmed funding and a separate regression explicitly requires refusal
of unconfirmed ancestor promotion. Initial strict Clippy map_entry failure fixed
without lint allowances. First rustfmt launch lacked explicit RUSTUP_HOME and
attempted toolchain sync; stopped that process and used the already installed
host toolchain with full process settings above. No infrastructure repair claimed.

Remaining SAME-owner source: combine this exact BDK post with detached Stock
accept_transfer and precise TXO/pending-script/receipt/status changes into a
COMPLETE expected snapshot. Check every local External input's RGB obligations;
never silently consume an unrelated allocation. Then bind one opaque preparation
to SAME-engine/source/observation, intent/debit-first one-shot application, full
actual/post equality, authenticated journal CAS/read-back, served acknowledgement
and journal-first ambiguous recovery. Current full consumer remains unproven and
unwired. Do not re-pin facade/wallet until the grouped consumer is ready.

Native --tests WASM-only integration import failure and SDK Clippy FAIL40 remain
unresolved; not rerun or claimed fixed. Chief retains grouped SDK default+TEST,
strict SDK Clippy/WASM/ABI/artifact identity/native-IDB/browser checks. Original
construction/live and SAME-case seller/buyer/fee BTC+RGB accounting, items5/16/18/19,
R1/R4, immediate-kill and required live reverse remain OPEN; R3 unavailable.
THE-872 source hold and THE-945 BOTH source+readiness holds remain. No financial
replay, acceptance uplift, actual GitHub APPROVE or merge. Rollback source only;
preserve original checkpoints, all financial journals and consumed obligations.
