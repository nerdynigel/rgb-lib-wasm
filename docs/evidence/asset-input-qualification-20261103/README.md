# Native RGB input qualification — 3 November2026

The new asset_spend_view method reports every currently eligible confirmed
fungible carrier of one canonical ContractId. It uses actual BDK prevouts, the
existing native original-operation fence, settled DB allocations and every
contract/assignment in Stock. DB/Stock amount disagreement, malformed originals,
wrong genesis/network, unsafe or overflowing amounts fail closed. Other assets
or inflation rights cannot burn in a trade. Missing, spent, pending, unconfirmed
and immature carriers are unavailable. No wallet/address/Stock/DB/persistence
state changes, signing, preparation, broadcast, new cryptography, dependency or
backup-format changes occur. Caller-held inputs still require the shared wallet
journal, and observation is not spending authority.

199native tests pass, including10new actual local BDK/DB/Stock graph cases. They
cover fragmented same-contract inputs, exact scripts/sats and values above
JavaScript's safe integer, state immutability, native original/missing-original
holds, unknown/ticker/mainnet refusal, overflow/amount disagreements, hidden
Stock, other contracts/inflation rights and pending/spent/unconfirmed/coinbase
inputs. Strict Clippy, formatter and wasm32 library compilation pass.
These are unfunded synthetic local graphs; full Max/native signing/settlement,
secure-facade browser execution and independent release acceptance remain open.
Exact command output/source digests are retained in RESULT.json and *.txt.
