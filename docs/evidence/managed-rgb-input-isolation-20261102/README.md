# Managed RGB input isolation — release-blocking candidate repair

Review during wallet integration found that the generic signer/broadcaster
refused only the managed original txid. A different transaction could therefore
reach those generic helpers with the same held RGB or vanilla fee input. The
application journal's exclusions did not replace native protection. This gap was
found in the unreleased ordinary-RGB candidate, not through an actual financial
operation or on the installed Lab engine, which has no managed ordinary send.

Native managed_rgb_input_holds now collects every original RGB and fee outpoint.
Generic sign/broadcast refuse any transaction intersecting that set before BDK
signing, network access or suspension. The exact managed original remains refused
even after cancellation. Consistent native unsigned-cancelled or confirmed-conflict
records can retire their input holds; contradictory/missing native history fails
closed. No expiry/missing ACK releases an input. Original records stay retained.

Dedicated managed signing and broadcast also refuse overlaps with other managed
originals. The native prepare selector unions these holds with the caller's
separate journal exclusions, protecting fee inputs even if a caller omits them.
A pre-existing overlapping unsigned preparation can still be explicitly cancelled;
no automatic replacement, re-signing, cancellation or financial retry is introduced.
Nonmanaged atomic external sends retain their existing separate adapter behavior.

133complete native units PASS, including16ordinary-transfer cases. New tests
exercise two independently held inputs (RGB and fee), modified txid and generic
sign refusal, actual broadcast-future refusal before its first suspension,
unchanged artifacts, disjoint-transaction compatibility and overlapping managed
preflight refusal. Consistent unsigned retirement unblocks the other original;
corrupt cancellation attempts remain refused. Fixtures are synthetic preflight
transactions, not valid funded RGB sends or on-chain evidence.
Strict library clippy (-D warnings), formatter/source diff and locked wasm32
library check PASS. Raw logs/hashes are retained here.

SDK aec9e8a4e549e75b361dec3a5eb86c455ceaad0a/private package currently includes
the earlier native a552139a08c6a12cb941810b552ec06e0abc1922. It must NOT be
promoted as repaired: explicitly repin/rebuild/review/retest this source first.
Its earlier guard tests remain valid only for their stated scope. No consumer
pin, Lab service, funding/grant, financial state or main merge changed here.
Installed root4fc279d7/wallet3c5d121/SDKe64c5893 remains unchanged. Complete
ordinary wallet runtime/task integration and funded recovery/spend-back remain open.

Compatibility: native parenta552139a08c6a12cb941810b552ec06e0abc1922;
btcx e21955589e55b9adba09deabb0ae4f8170cba796;
wallet base3c5d12112b397d344159368fa6994bec333dfb02 (journal work unpromoted);
protocolb626da0c337875e9bb75c46079cd85bdc0a21369;
signer074de663746b654bbd1cb9756e34c531101edec8;
native rust-lightning0939f7751fb876e02caec9cd0a778794b81eee9e.
No shared market wire change. Explicit SDK/consumer revision and acceptance required.
