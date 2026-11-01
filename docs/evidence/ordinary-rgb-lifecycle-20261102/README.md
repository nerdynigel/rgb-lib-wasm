# Ordinary RGB lifecycle — native implementation checkpoint

Adds dedicated broadcast_wallet_rgb_send, observe_wallet_rgb_send and
cancel_wallet_rgb_send over the retained wallet-owned preparation/signing path.
The financial transaction is the original standard BDK/RGB external send; this
does not introduce another settlement engine or alter the atomic swap profile.

Broadcast requires the exact retained fully witnessed signed PSBT, current
REGTEST genesis, canonical operation/request/batch and unexpired terms. Restored,
previously broadcast, unsigned or changed originals are refused. It first checks
for the original on the indexer, posts retained consignments through the existing
idempotent transport, flushes metadata, revalidates and commits broadcast_attempted
BEFORE one indexer broadcast call. Expiry is rechecked after that durable flush.
Any missing response leaves the original/attempt held. No financial retry or
generic broadcast acknowledgement fallback is used.

Observation never signs or broadcasts. It checks the actual original transaction
against its exact prepared transaction and, when present, retained signed witness
bytes. It reuses existing exact external finalization/Stock consumption and only
the matching outgoing native batch's confirmation path; unrelated pending sends
are not refreshed. It reports observed/depth/settlement_verified separately, with
integer-string confirmation depth. Missing indexer data is not failure, input
release or settlement. A historical Settled batch alone is insufficient when
current depth is absent. Receiver validation/spendability is separately required
in the customer journey; outgoing native confirmation is not atomic swap proof.

Cancellation is explicit and requires the exact original preparation, no signature,
no signing/broadcast attempt and no restored-backup barrier. Expiry alone does
not release inputs. It marks the original native batch Failed and retained
external status Cancelled without consuming Stock. Repeated exact cancellation
is idempotent. Getter rejects contradictory cancelled/signed/attempted state;
generic failed-transfer deletion cannot erase any managed original/history.
Snapshot terminal-state nonregression includes Cancelled. Generic PSBT broadcast
and pending-send paths refuse managed operations before network use; original
explicit consignment metadata recovery remains available without financial replay.

131complete native library unit tests PASS, including14focused ordinary-transfer
cases. Final strict library clippy (-D warnings), formatter, diff check and locked
wasm32 library check PASS. The broadcast-state fixture intentionally contains an
invalid synthetic signature; it tests exact-byte/state refusals only. The generic
broadcast regression polls the actual future once and proves it refuses before
network access/suspension, preserving artifacts. Cancellation tests cover expired
unsigned state, retained readability, cleanup refusal and revival/signing refusal.
No funded send, storage-fault browser acceptance, live broadcast/recovery or
recipient spend-back is claimed by these tests. Logs/hashes are retained here.

Compatibility: native parent0ef0d687f544fb687c267a89b8571e8d32a258f5;
btcx8ec4abea50f9fd4f7267ceea9d12cbd53bde3be0;
wallet3c5d12112b397d344159368fa6994bec333dfb02;
protocolb626da0c337875e9bb75c46079cd85bdc0a21369;
RLN SDK consumer stillfbb6480 (preparation only), installed9da3a4dc/RGBe967aa95/
SDKe64c5893. Signer074de663746b654bbd1cb9756e34c531101edec8 and native
rust-lightning0939f7751fb876e02caec9cd0a778794b81eee9e unchanged.
No shared market wire change; deliberate SDK/wallet/storage profile updates and
actual secure customer transfer/recovery tests are still required before release.
No Lab mutation, grant, funding, reset, ambiguous replay or main merge.
