# Optional REGTEST selected-input control candidate

Protocol1.29, exact5ce0315e95e14d5b4aa438837a3175a37565fed9 defines the contract
before consumers. This source candidate introduces `prove_inventory_control`.
It qualifies exact current native Bitcoin/RGB inputs and uses pinned patched
bip3220.0.11 to construct standard synthetic message transactions, the existing
BDK signer and upstream standard verification. No arbitrary message/PSBT/path,
private key export, real spending transaction, preparation, network request,
reservation, persistent state or broadcast. No production/Lab pin promoted.

The new control proof binds origin/session/owner hashes, nonce, immutable bounded
expiry, canonical pair, chain checkpoint, standard consignment hash and each
selected outpoint/script/sats/units. Browser local consent and its existing shared
input lock/holds remain mandatory at the future consumer entry. Native does not
assert an authenticated origin merely because a hash is supplied. Host nonce
consumption and independent Core/RGB/current reservation checks remain mandatory.
This is NOT complete verified general inventory, readiness, signing approval or
an accepted product release.

Dependency audit adds only bip3220.0.11, base640.23.1, snafu0.9.2 and
snafu-derive0.9.2. No existing package is removed/upgraded or has fields changed,
except this root package adding bip322. The patched release includes the
upstream P2WPKH key-binding repair; only P2TR key-path SIGHASH_DEFAULT with one
64byte witness signature is admitted here. No custom cryptography.
Source references:
https://rustsec.org/advisories/RUSTSEC-2026-0334.html
https://github.com/rust-bitcoin/bip322
https://bips.dev/322/

Tests use actual BDK/Stock/DB local graphs and signatures, not real chain funds.
Both keychains and fragmented standard RGB proof/state pass; changed context,
foreign key/input, stale/invalid/overflow claims and existing Native pending or
RGB holds refuse. Exact DB/Stock/BDK staging/artifacts/persistence sequence remain
unchanged. Canonical messages match independent protocol vectors. Full checks
and secure-SDK/browser/current wallet integration evidence belong to the exact
consumer source checkpoint; no component substitutes for financial acceptance.
