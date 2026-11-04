# Native complete-set Bitcoin self-recovery — 4 November 2026

Implemented Native API candidate, not deployed or actual financial acceptance.
The browser secure facade, wallet integration, actual async IndexedDB/restart/
interruption/file recovery and funded customer proof remain required.

`BitcoinRecoveryRequest` binds the exact unsigned standard PSBT, REGTEST network,
integer fee rate/cap and bounded original approval. Strict parsing requires1–32
canonical ordered unique inputs, one P2TR output, exact witness UTXOs, no unrelated
maps/metadata, version2/locktime0/final sequences and checked integer cost. Every
input must currently qualify against real BDK/DB/Stock/pending Native operations.
The return is independently Native-derived on the vanilla keychain. RGB-bearing
inputs, foreign outputs, unknown costs, dust and excessive fees refuse.

The new `sign_bitcoin_recovery` method commits one Native attempt before the
existing BDK signer, then the exact raw original before acknowledgement. It
checks actual REGTEST/genesis, current input state, expiry and that the original
transaction is unobserved before signing. `bitcoin_recovery_data` only reads
retained bytes, with an explicit completeness flag and NOT_SETTLEMENT scope.
Expiry and restoration never grant another signature. A failed/partial signer
may leave incomplete bytes; they remain held and cannot be silently replaced.

`broadcast_bitcoin_recovery` submits only the exact retained verified original,
with fresh bounded explicit dispatch consent and a durable attempt before the
network primitive. Lost replies/errors require observation rather than another
dispatch. The existing Bitcoin broadcast/graph primitives remain authoritative.
Generic signing, Bitcoin selection, RGB/fee operations and collaborative partial
signing see these holds; an own-recovery exemption cannot exempt another original.
The Native record is separate from RGB receipts and never reports a swap settled.

The established BIP341/secp256k1 verifier is shared with collaborative partials;
all input signatures and immutable transaction/global/input/output metadata are
checked. Only independently matching added public Taproot derivation metadata
is normalized. Existing BDK security flags, crypto and dependency versions remain.

Managed snapshot5/Native encrypted backup7 reject old-header downgrades, corrupt
or mixed-purpose artifacts, stale flush acknowledgements, dropped/replaced
requests/originals and lost attempt/restore flags. Imported attempts are marked
restored; original reads remain available, fresh signing/dispatch refuses. Old
supported snapshot and encrypted backup2–6 formats retain their meaning. Explicit
legacy3–6 encrypted round trips remain tested. No client state, Stock, reservation
or incident ledger is reset, deleted, rebuilt or replayed.

CHECKS.json and full logs retain224Native tests, strict established library Clippy,
formatter and wasm32 library compilation. Eight added tests use real invented
local BDK graph/signature and codec/KDF fixtures:1/2/32inputs, exact return/fees,
foreign/missing/reordered/duplicate/value/script mutations, missing/invalid each
signature, Native hold isolation, one signing/dispatch attempt, restore fences,
append-only snapshot writes and exact encrypted-original preservation/downgrade
refusal. These tests do not exercise the async public API with browser IndexedDB,
Core broadcast, real RGB state, a customer wallet or atomic settlement. Source
and evidence hashes are retained; SDK/consumer pins and final proof remain open.

The first migration-test compile failed because a test-only fixture module was
visible only to its immediate parent; visibility was corrected for crate-wallet
backup/snapshot tests. No assertions or production safety checks were weakened.

Base Native sourcebc28832fa3f4b377627aeede9659ea3f77a5d23c; paired installed
SDKadd6ff9/walletfe28b4c/Root e5e73f0a remain unchanged. The next exact secure-SDK
pin must expose sign/read/dispatch APIs, verify browser origin/session/encrypted
persistence, then connect wallet sign-once/read-original/reconcile and RGB whole-set
recovery. All four Root financial gates, independent acceptance, FigCraft/customer/
staff/fees/prices/comments and main-merge restrictions remain mandatory.

No mainnet, real assets, nonzero execution fees, new cryptography/dependencies,
server keys, financial dispatch, Lab mutation or main merge in this checkpoint.
