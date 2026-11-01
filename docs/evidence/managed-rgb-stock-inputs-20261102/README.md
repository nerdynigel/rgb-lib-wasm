# Managed ordinary RGB sends — unchanged Stock input guard

Before ordinary RGB signing and broadcast, compare every currently assigned RGB
state on the original Bitcoin inputs with the exact immutable input allocations
retained by standard native preparation. Use the real Stock contract assignments;
no reconstructed graph, private Stock replacement, balance projection or new
cryptography. Any added contract, changed/consumed state, non-owner assignment,
missing/corrupt original, duplicate outpoint or checked-integer overflow refuses.
The registry scan stops at4097entries and rejects more than4096contracts.

Signing checks before its attempt and again after the durable attempt flush;
broadcast checks before consignment posting and again after its attempt flush.
A known original already on chain takes the existing observation path. No new
financial retry, cancellation, input release or fee collection is added. Public
ordinary-transfer JSON remains unchanged. The public unspent API already filters
its own outgoing reservation rows; no client balance workaround is introduced.

134full native unit cases PASS. After the final bounded-inventory adjustment,
17focused wallet_transfer cases, strict library clippy(-Dwarnings), formatter
and locked wasm32 library check PASS. The new test uses a synthetic standard
preparation record: exact u64 facts, overflow/missing/misbound data refusal and
refusal when the actual Stock has no corresponding input state. This is not a
funded positive Stock/send/recovery proof. Existing preservation/one-attempt
assertions remain. Fixture transfer metadata was made explicit; no real key or
RGB consignment is embedded.

Previous native9d42245bd1099a03ec2bbfb41ade00d710aac329 remains the parent.
The private SDK built from RLNea7820c/native9d is historical evidence and is not
a release of this guard. SDK must be exact-pinned/rebuilt/reviewed and the wallet
consumer integrated and tested on fresh isolated REGTEST state. Lab root1106cc58,
wallet3c5d121, SDKe64c5893, private state, financial hold and main are unchanged
by this source change. No funded RGB send, settlement or independent acceptance
is claimed.

Compatibility:
- btcx-wallet-protocol:b626da0c337875e9bb75c46079cd85bdc0a21369
- btcx:bf070ac2ff48ba4db5c176e660162a9263575c59 (Lab1106cc58)
- btcx-wallet:337a2626771f29538d1bf53a374a74f5dd002d27 plus unpromoted runtime work
- RLN:ea7820c3feffd2d0ae75c40685911ee86bf532d1 plus unpromoted public decoder
- signer:074de663746b654bbd1cb9756e34c531101edec8
- native rust-lightning:0939f7751fb876e02caec9cd0a778794b81eee9e
- breaking shared protocol change:NO
- peer repo update required:YES, exact SDK pin/build and wallet runtime acceptance
