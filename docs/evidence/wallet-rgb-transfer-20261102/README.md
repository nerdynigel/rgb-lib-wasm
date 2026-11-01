# Ordinary RGB preparation — REGTEST work in progress

Native preparation and exact operation lookup are implemented. This is NOT a
complete Send feature, WASM/funded acceptance, new Lab release or main acceptance.
The installed Lab remains root027b2d3b/wallet4e5191f/RLN9da3a4dc/RGBe967aa95.

The legacy send_begin path consumes real Stock while preparing; send_end removes
most original metadata before network completion. Do not connect the new customer
Send task to those methods. The new prepare_wallet_rgb_send uses existing BDK
construction and standard RGB color_psbt/prepare_external_rgb_send to reserve
inputs and persist complete PSBT/fascia/consignment/recipient/request data while
leaving Stock unconsumed. No new cryptography, shadow state or market custody.

Explicit request binds UUID, exact ContractId, network-bound witness recipient,
RGB units, delivered BTC carrier, integer fee rate, confirmation depth, expiry and
other durable input holds. Strict schemas/size bounds/u64 wire strings, checked
candidate totals, fresh sync, REGTEST genesis and confirmed owned inputs precede
preparation. One settled NIA asset only; mixed assets and unsupported beneficiaries
fail closed. Vanilla fee inputs exclude every RGB DB outpoint and caller hold.
Identical retries recover the original operation before selection; changed terms,
failed original or duplicate operation bindings in corrupted state refuse.

The binding is included in the existing durable external artifact BEFORE its
first flush. Lookup never regenerates a transaction. Legacy generic sign_psbt
and send_end refuse managed artifacts before signing/destructive removal. Existing
atomic preparation keeps wallet_transfer=None and its previous semantics. Old
artifacts without the optional field still deserialize. Full backup round-trip
retains the original request and both attempt flags (future lifecycle work).

124native library tests PASS, including9new validation/identity/storage/guard cases,
strict clippy/format PASS. First clippy attempt's byte-slice warning is retained;
it was corrected without suppressing checks. No dependency/lockfile change.
Tests are native/unfunded; real WASM construction, SDK lifecycle, wallet journal,
explicit sign/once broadcast/confirmed-only reconciliation/cancellation, backup
reader gating and actual customer transfer/recovery remain to implement and test.
No financial dispatch, reset, admission, balance credit or current peer pin changes.

Compatibility:
- btcx-wallet-protocol:b626da0c337875e9bb75c46079cd85bdc0a21369 (unchanged)
- btcx:26a77caf7aa72186cc5360999f652252b66c6b0e; installed027b2d3b6b91465106ae3c7e769a95ca3e18d537
- btcx-wallet:4e5191f293348cb34dfe70c09551b649d4b7b23a
- RLN:9da3a4dc9bced78b5559d718e60b5c54d26f6857; RGBthiscommit(parent e967aa95f7e66371799a46dd03a61f8409c08e91), not promoted
- signer:074de663746b654bbd1cb9756e34c531101edec8
- rust-lightning:0939f7751fb876e02caec9cd0a778794b81eee9e
- breaking protocol change:NO (wallet-owned SDK capability, no shared market interface)
- peer repo update required:YES, explicit SDK facade/build and consumer validation before promotion
