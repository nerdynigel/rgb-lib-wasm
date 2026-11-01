# Preserve prepared external sends during automatic refresh

Actual ASM REGTEST customer testing reproduced a WASM panic at
src/wallet/online.rs3584:74 on parent6a8cb2fbe68d53940a940c2e9abdddc25b47e40e.
Initiated is pending but is not an ACK/confirmation refresh state. An incoming
filter attempted to convert it with expect("pending status"), trapping the
future and leaving the wallet engine/input queue held. Discovery, balances and
signing review then waited indefinitely. No original swap was broadcast.

Normal refresh now accepts only the two existing refreshable statuses before
matching direction/status filters. Initiated proposals and terminal states are
ignored for both filtered and unfiltered refresh. This does not advance, expire,
cancel, consume, sign or broadcast a prepared send, nor release a reservation.
The established explicit finalize/confirmed-conflict paths remain unchanged.
No asset, wire, persistence or cryptographic format changes.

Validation: cargo fmt --all --check; cargo clippy --locked --lib -- -D warnings;
cargo test --locked --lib,115 passed, including two regressions covering all
five statuses, both directions, empty/incoming/outgoing filters and mismatches.
Actual retained-wallet WASM rebuild/read-back is still required. No new atomic
settlement or mainnet acceptance is claimed. Evidence is retained in BTCX PR252.

Compatibility:
- btcx-wallet-protocol: b626da0c337875e9bb75c46079cd85bdc0a21369, unchanged
- btcx: feat/asm-product-lab-release-20261101, evidence66bc3c32; installed1c0a0a35
- btcx-wallet: feat/asm-product-lab-release-20261101, parenta03fe8ac; pin update pending
- RLN: parenteb6187daf441e883508353ecd6ad291991c10f07; exact RGB pin migration required
- signer: 074de663746b654bbd1cb9756e34c531101edec8, unchanged
- rust-lightning: 0939f7751fb876e02caec9cd0a778794b81eee9e, unchanged
- breaking protocol change: NO
- peer repo update required: YES, RLN locked RGB source, rebuilt SDK and coordinated wallet/market artifacts
