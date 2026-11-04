# Native assignment-free Bitcoin spending candidate

Base: d3b9b13c0076303060926a2776b0c28e63e4cdb5. Private REGTEST candidate;
not deployed and not financial or independent security acceptance.

The actual R ordinary RGB self-transfer left 96,233 sats on an owned RGB
descriptor output without an RGB assignment. The old vanilla-only spender
excluded it. An empty display allocation alone cannot make such an output safe.

`bitcoin_spend_view` qualifies confirmed mature P2TR outputs from both native BDK
keychains against the native database, all actual Stock contracts/assignment
types, pending witness/blinded receive promises and retained outgoing originals
(including fee inputs). Corrupt or ambiguous native state refuses. Its output is
an observation, not authorization. Additional browser holds remain mandatory.

Bounded REGTEST-only preparation uses the existing BDK builder; signing and
one-shot broadcast recheck genesis, sync, expiry, exact owned input values and
Stock/native holds. BTC change returns to the vanilla descriptor. The host must
persist the refusal reservation, exact signed original and one attempt before
the corresponding side effect. Lost replies require observation, never replay.
No cryptography, dependencies, persistence schema or asset identity changed.

New ordinary RGB preparations use an explicit RGB change carrier and a separate
vanilla Bitcoin change output. Existing retained originals are untouched.
Read-only `bitcoin_rgb_fee_inputs` exempts only its exact prepared managed RGB
original for independent review; ordinary BTC methods have no such exemption.

Validation: `cargo test --locked --lib` passes 154 tests; `cargo clippy --locked
--lib -- -D warnings`, `cargo fmt --all --check` and `git diff --check` pass.
Sixteen new tests use real local BDK graphs, native DB and Stock, including hidden
NIA and IFA inflation assignments, pending promises, late state changes, malformed
rows/PSBTs, foreign scripts, confirmation/maturity, exact original fee holds and
own-review exemption isolation. These fixtures do not broadcast or prove swaps.
Retained command logs and source digests accompany this checkpoint.

Next: exact-pin the secure SDK, finish browser ordinary/send/shared swap funding
and recovery integration, then bounded actual native/customer acceptance. Existing
incidents, unresolved obligations, financial hold and main remain unchanged.
