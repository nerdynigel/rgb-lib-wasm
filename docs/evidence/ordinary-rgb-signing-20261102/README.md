# Ordinary RGB original signing — incomplete native candidate

Adds sign_wallet_rgb_send(online, operation_id, exact_prepared_psbt). It requires
REGTEST, a unique bound canonical request, the unchanged prepared PSBT/unsigned
transaction, the native Initiated batch and the approved expiry. Restored or
previously attempted/signed records cannot request a new signature. The real
indexer genesis is independently checked; expiry is revalidated after that await
and after the pre-signing flush. The binding's attempt flag commits BEFORE the
existing BDK signer runs. No caller-controlled signing options or new crypto.

Every returned PSBT byte, including a partial/error outcome, is retained and
flushed before finalization/classification. Unknown storage/signing outcomes
remain held; no automatic new signature. Getter exposes original attempted bytes
and distinguishes a complete signed transaction from incomplete state. Incomplete
PSBTs remain readable for recovery; they never become a ready-to-broadcast result.
Only internal wallet modules gain access to the existing signer helper; the
public generic signer still refuses managed preparations.

128complete native unit cases, strict library clippy, formatter and
cargo check --locked --target wasm32-unknown-unknown --lib PASS. Tests exercise
exact reviewed-byte binding, attempt/restored/expiry refusals, no-mutation and
retention of incomplete original bytes. The first native preflight case called
the WASM-only clock and panicked; it is retained. The preflight now takes an
explicit Unix-second observation: production samples the real clock at each
boundary, and native tests provide a deterministic time. No assertions weakened.

These tests do NOT execute a funded signature, prove WASM storage-failure ordering
or demonstrate a transfer. The SDK still pins the earlier preparation candidate;
there is no new signing facade/consumer promotion yet. Once-broadcast, observation,
unsigned cancellation, complete wallet journal/backup/UX and actual customer
send/receive/recovery/spend-back remain work to finish. Native recovery format
52707e23 and its pending restored barrier are prerequisites, not completed delivery.

No Lab grant, reset, signing, broadcast or main merge. Installed4fc279d7 /
wallet3c5d121 / RLN9da3 / RGBe967 / SDKe64 remains the same held GUI candidate.

Compatibility:
- btcx-wallet-protocol:b626da0c337875e9bb75c46079cd85bdc0a21369
- btcx:99330c13682cf3a64fa68cd33f23ee14c3ff4939
- btcx-wallet:3c5d12112b397d344159368fa6994bec333dfb02
- RLN:privatefb5f6cf (preparation-only), installed9da3a4dc
- RGB:thiscommit,parent52707e23c12b37ed4b2c1d7b452af07afbb4e9de
- signer:074de663746b654bbd1cb9756e34c531101edec8
- rust-lightning:0939f7751fb876e02caec9cd0a778794b81eee9e
- breaking protocol change:NO; wallet-owned native API, no shared market contract change
- peer repo update required:YES, deliberate exact SDK/consumer/storage profile update and acceptance
