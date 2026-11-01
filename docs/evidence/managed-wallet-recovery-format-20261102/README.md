# Managed ordinary RGB recovery format — incomplete candidate

Before enabling ordinary RGB signing, new managed snapshots need a versioned
wrapper. Older flat-snapshot readers fail to decode the required db/sequence
instead of dropping unknown wallet-transfer attempt flags. Ordinary snapshots
without managed transfers retain their prior format. New readers accept those
legacy snapshots but reject unversioned managed, hybrid and unknown-version data.

Backup version3 wraps its encrypted payload as well as its public header. Old
flat readers refuse the encrypted plaintext even when the public version is
changed to2. New readers retain compatible complete v2 backups; unknown and
unversioned managed payloads fail closed. Local and VSS recovery share the
versioned decoder. Existing AEAD/Scrypt primitives/dependencies are unchanged.

A restored pending managed operation receives a durable restored barrier: a
backup can predate a later signature/broadcast elsewhere. Future dedicated send
APIs must refuse new sign/broadcast/cancel when restored; original observation/
confirmed-conflict recovery stays possible. Lost-restore-ACK comparison permits
only monotonic barrier growth (and existing public derivation/bookkeeping rules),
never different original bytes, financial records, attempts or Stock.

Managed snapshots cannot silently acknowledge a stale write, drop original
records/signatures/consignments, roll back attempt/restored flags, or regress
terminal lifecycle state. Checked sequence advance refuses exhaustion without
wrapping. Existing non-managed stale-snapshot handling remains unchanged.

127complete native unit cases PASS, including encrypted version-header downgrade,
legacy v2 payload compatibility, original signing/consignment/attempt retention,
restore barrier, required snapshot wrapper, stale-write/original-loss refusals,
and sequence exhaustion. Strict cargo clippy --locked --lib -- -D warnings and
cargo fmt PASS. Extended --lib --tests strict clippy was also tried: it fails on
pre-existing test-only Arc/non-Send and host-unused wasm integration imports.
That failure is retained; no assertions/warnings were suppressed. No WASM IDB,
funded transfer, signature, broadcast or complete recovery acceptance is inferred
from these native tests. New file formats still require browser/migration checks.

This is a private engine candidate. No wallet consumer, SDK package or Lab pin
is promoted. Installed root4fc279d7/wallet3c5d121/RLN9da3/RGBe967/SDKe64c5893
remains under existing review-only authority. Ordinary RGB lifecycle/UX and actual
customer/recovery/spend-back proof remain OPEN. No new grants, reset, unknown
replay, mainnet, real funds or main merge.

Compatibility:
- btcx-wallet-protocol:b626da0c337875e9bb75c46079cd85bdc0a21369
- btcx:99330c13682cf3a64fa68cd33f23ee14c3ff4939
- btcx-wallet:3c5d12112b397d344159368fa6994bec333dfb02
- RLN:private preparation SDKfb5f6cf, installed9da3a4dc (not promoted)
- RGB:thiscommit,parentfbb648070fbc3e68f5ee9d6875a3ab3ec77880a5
- signer:074de663746b654bbd1cb9756e34c531101edec8
- rust-lightning:0939f7751fb876e02caec9cd0a778794b81eee9e
- breaking protocol change:NO; engine backupv3 / managed-snapshotv1 are explicit new storage profiles
- peer repo update required:YES; deliberate SDK pin/outer backup profile/migration and financial review before promotion
