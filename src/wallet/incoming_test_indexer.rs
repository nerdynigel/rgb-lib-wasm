//! Native TEST-only loopback Esplora fixture. Real HTTP/client/validator calls,
//! synthetic chain data: this is not live settlement or durable journal proof.
use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Fault {
    None,
    Witness,
    Tip,
    Inclusion,
    Network,
    MissingPrevious,
}

struct Server {
    address: String,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    injected: Arc<AtomicBool>,
}
impl Server {
    fn new(routes: HashMap<String, Vec<u8>>, fault: Fault, witness_path: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let injected = Arc::new(AtomicBool::new(false));
        let injection = injected.clone();
        let worker = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(20);
            let mut tips = 0;
            let mut statuses = 0;
            while !stopping.load(Ordering::SeqCst) && Instant::now() < deadline {
                let (mut stream, _) = match listener.accept() {
                    Ok(pair) => pair,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(error) => panic!("test listener failed: {error}"),
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0u8];
                    if stream.read(&mut byte).unwrap() == 0 {
                        break;
                    }
                    request.push(byte[0]);
                    assert!(request.len() <= 8192);
                }
                let request = std::str::from_utf8(&request).unwrap();
                let mut line = request.lines().next().unwrap().split_whitespace();
                assert_eq!(line.next(), Some("GET"), "observer must never POST");
                let path = line.next().unwrap();
                let mut body = routes
                    .get(path)
                    .unwrap_or_else(|| panic!("unexpected indexer path: {path}"))
                    .clone();
                if path == "/blocks/tip/height" {
                    tips += 1;
                    if fault == Fault::Tip && tips > 1 {
                        body = b"21".to_vec();
                        injection.store(true, Ordering::SeqCst);
                    }
                }
                if path.ends_with("/status") {
                    statuses += 1;
                    if fault == Fault::Inclusion && statuses > 1 {
                        body = br#"{"confirmed":false}"#.to_vec();
                        injection.store(true, Ordering::SeqCst);
                    }
                }
                if fault == Fault::Network && path == "/block-height/0" {
                    body = bitcoin::blockdata::constants::genesis_block(BdkNetwork::Bitcoin)
                        .block_hash()
                        .to_string()
                        .into_bytes();
                    injection.store(true, Ordering::SeqCst);
                }
                let missing = fault == Fault::MissingPrevious
                    && path.ends_with("/raw")
                    && path != witness_path;
                if missing {
                    injection.store(true, Ordering::SeqCst);
                }
                if fault == Fault::Witness && path == witness_path {
                    // Preserve txid/outputs while changing exact witness bytes.
                    let mut tx: BdkTransaction = bitcoin::consensus::deserialize(&body).unwrap();
                    tx.input[0].witness.push([1u8]);
                    body = bitcoin::consensus::serialize(&tx);
                    injection.store(true, Ordering::SeqCst);
                }
                let status = if missing { "404 Not Found" } else { "200 OK" };
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(&body).unwrap();
            }
        });
        Self {
            address,
            stop,
            worker: Some(worker),
            injected,
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.worker.take().unwrap().join().unwrap();
    }
}

fn with_indexer<T>(
    wallet: &mut Wallet,
    receipt: OutPoint,
    transaction: &BdkTransaction,
    fault: Fault,
    action: impl FnOnce(&tokio::runtime::Runtime, &mut Wallet, Online) -> Result<T, Error>,
) -> Result<T, Error> {
    let genesis = bitcoin::blockdata::constants::genesis_block(BdkNetwork::Regtest).block_hash();
    let hash = BlockHash::from_str(&"01".repeat(32)).unwrap();
    let mut routes = HashMap::from([
        ("/block-height/0".into(), genesis.to_string().into_bytes()),
        ("/block-height/10".into(), "05".repeat(32).into_bytes()),
        ("/block-height/20".into(), hash.to_string().into_bytes()),
        ("/blocks/tip/height".into(), b"20".to_vec()),
        (format!("/tx/{}/status", receipt.txid), serde_json::to_vec(&serde_json::json!({
            "confirmed":true, "block_height":20, "block_hash":hash.to_string(), "block_time":1_700_000_000u64
        })).unwrap()),
    ]);
    for input in &transaction.input {
        let point = input.previous_output;
        let previous = if let Some(tx) = wallet.bdk_wallet.get_tx(point.txid) {
            tx.tx_node.tx.as_ref().clone()
        } else {
            super::incoming_bdk::tests::foreign_funding()
        };
        assert_eq!(previous.compute_txid(), point.txid);
        routes.insert(
            format!("/tx/{}/raw", point.txid),
            bitcoin::consensus::serialize(&previous),
        );
    }
    let witness_path = format!("/tx/{}/raw", receipt.txid);
    routes.insert(
        witness_path.clone(),
        bitcoin::consensus::serialize(transaction),
    );
    let server = Server::new(routes, fault, witness_path);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let online = runtime.block_on(wallet.go_online(true, server.address.clone()))?;
    let result = action(&runtime, wallet, online);
    assert_eq!(
        server.injected.load(Ordering::SeqCst),
        fault != Fault::None,
        "negative case must reach its intended network fault"
    );
    result
}

pub(super) fn observe(
    wallet: &mut Wallet,
    recipient: &str,
    contract: ContractId,
    receipt: OutPoint,
    consignment: &[u8],
    transaction: &BdkTransaction,
    fault: Fault,
) -> Result<super::incoming_observation::FrozenIncoming, Error> {
    with_indexer(
        wallet,
        receipt,
        transaction,
        fault,
        |runtime, wallet, online| {
            runtime.block_on(wallet.observe_pending_witness_receive(
                online,
                recipient,
                contract,
                receipt,
                1107,
                consignment,
                transaction,
            ))
        },
    )
}

/// Exercise the public Rust embedding boundary; no FrozenIncoming constructor
/// or prospective checkpoint supplied by the host can enter this path.
pub(super) fn apply_exact(
    wallet: &mut Wallet,
    recipient: &str,
    contract: ContractId,
    receipt: OutPoint,
    consignment: &[u8],
    transaction: &BdkTransaction,
) -> Result<(serde_json::Value, serde_json::Value), Error> {
    with_indexer(
        wallet,
        receipt,
        transaction,
        Fault::None,
        |runtime, wallet, online| {
            let original = super::incoming_observation::complete_source(wallet)?;
            wallet.consumer_owned_persistence = false;
            assert!(
                runtime
                    .block_on(wallet.prepare_confirmed_pending_witness_receive(
                        online.clone(),
                        recipient,
                        contract,
                        receipt,
                        1107,
                        consignment,
                        transaction,
                    ))
                    .is_err(),
                "legacy persistence must refuse the public prepare boundary"
            );
            wallet.consumer_owned_persistence = true;
            assert_eq!(
                super::incoming_observation::complete_source(wallet)?,
                original
            );
            let prepared = runtime.block_on(wallet.prepare_confirmed_pending_witness_receive(
                online.clone(),
                recipient,
                contract,
                receipt,
                1107,
                consignment,
                transaction,
            ))?;
            prepared.require_original()?;
            drop(prepared);
            assert_eq!(
                super::incoming_observation::complete_source(wallet)?,
                original,
                "dropping a public preparation cannot consume any state"
            );
            let prepared = runtime.block_on(wallet.prepare_confirmed_pending_witness_receive(
                online,
                recipient,
                contract,
                receipt,
                1107,
                consignment,
                transaction,
            ))?;
            prepared.require_original()?;
            let prospective = serde_json::to_value(prepared.prospective_snapshot()).unwrap();
            let actual = serde_json::to_value(prepared.apply()?).unwrap();
            Ok((prospective, actual))
        },
    )
}
