# Compatible UTEXO hardening candidate — 2026-11-14

PARTIAL integration, not dependency promotion or financial acceptance.
Baseline `7467089467cbffb1e1ce1e4ee54d837c569b8abe`; inspected upstream dev
`eb562fcdd5b111d8efad85a7eba72c87e13a3d2c`. This branch is a minimal backport
onto the accepted ASM engine, **not** a build of the new upstream protocol.

Adaptations:
- `7843627c7d8fb4b66d74d6935e4fd8107b27dbff` (#114): public WASM metadata
  redacts the mnemonic; shared `get_public_wallet_data` supports the RLN facade.
  Internal signing data and the explicit secure-session backup ceremony remain.
- `3880d33da47ddb8f323316b7fd989b130fa68d94`: await `flush` after async UTXO,
  send, drain, inflate and witness updates; sync, refresh and fail retain partial
  progress before returning errors. Keep the accepted snapshot format and guards.
- `e11f286f28dcd4393528b181af7f071867f72bff`: Rustix 1.1.4 -> 1.1.6 for
  RUSTSEC-2026-0486. Other package versions remain fixed.

No schema, network, proxy, RGB protocol or wallet-state migration. In particular,
`allow_reckless_schemas` is not exposed and mainnet schema restrictions are retained.
The new proxy-v1 expiration API is incompatible with this proxy-v0.3 stack and is
not silently mapped onto the existing lifecycle.

Retain `create_transfers_from_fascia`, prepare/finalize, exact transaction matching,
transaction-indexed lookup, durable prepare/broadcast/finalize, restored-state
fences, input reservations and integer allocation checks. Upstream `color_psbt`
and `consume_fascia` alone do not replace these operations. Current upstream's
`color_psbt` also lacks this fork's checked additions and `vout >= len` refusal.
The new upstream MemStock/proxy-v1/RGB-0.11.2 format needs independently tested
snapshot compatibility before replacing the accepted stock/proxy-v0.3 engine.

Regression commands: `cargo fmt --all -- --check`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo test --workspace`, wasm32 workspace check,
`wasm-pack test --headless --firefox --release --lib -- upstream_durability_tests`,
and binding test `upstream_metadata_wasm`. New browser tests use real IndexedDB
without an economic fixture; they do not establish atomic settlement.
The BTCX consolidated report records actual outcomes and live acceptance holds.

Rollback: restore exact baseline and rebuild its RLN + wallet bundle; preserve
all wallet databases and immutable SDK namespaces. Never restore older financial
state over newer signed/broadcast obligations. No migration is required by this
candidate because stored formats are unchanged.
