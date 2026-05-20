mod common;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use taolk::event::{ConnState, Event};
use taolk::extrinsic::RemarkCallIds;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message as WsMessage;

type TestWs = WebSocketStream<tokio::net::TcpStream>;
type RpcScript = Vec<(&'static str, Value)>;

fn push_compact(out: &mut Vec<u8>, value: u32) {
    samp::scale::encode_compact(u64::from(value), out);
}

fn push_string(out: &mut Vec<u8>, value: &str) {
    push_compact(out, value.len() as u32);
    out.extend_from_slice(value.as_bytes());
}

fn push_strings(out: &mut Vec<u8>, values: &[&str]) {
    push_compact(out, values.len() as u32);
    for value in values {
        push_string(out, value);
    }
}

fn push_option_string(out: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(value) => {
            out.push(1);
            push_string(out, value);
        }
        None => out.push(0),
    }
}

fn push_field(out: &mut Vec<u8>, name: &str, ty: u32) {
    push_option_string(out, Some(name));
    push_compact(out, ty);
    push_option_string(out, None);
    push_strings(out, &[]);
}

fn push_type_header(out: &mut Vec<u8>, id: u32) {
    push_compact(out, id);
    push_strings(out, &[]);
    push_compact(out, 0);
}

fn push_type_footer(out: &mut Vec<u8>) {
    push_strings(out, &[]);
}

fn push_call_variant_type(out: &mut Vec<u8>, id: u32, variants: &[(&str, u8)]) {
    push_type_header(out, id);
    out.push(1);
    push_compact(out, variants.len() as u32);
    for (name, idx) in variants {
        push_string(out, name);
        push_compact(out, 0);
        out.push(*idx);
        push_strings(out, &[]);
    }
    push_type_footer(out);
}

fn push_composite_type(out: &mut Vec<u8>, id: u32, fields: &[(&str, u32)]) {
    push_type_header(out, id);
    out.push(0);
    push_compact(out, fields.len() as u32);
    for (name, ty) in fields {
        push_field(out, name, *ty);
    }
    push_type_footer(out);
}

fn push_primitive_type(out: &mut Vec<u8>, id: u32, primitive: u8) {
    push_type_header(out, id);
    out.push(5);
    out.push(primitive);
    push_type_footer(out);
}

fn metadata_hex_with_system_calls(remark: Option<u8>, remark_with_event: Option<u8>) -> String {
    let mut variants = Vec::new();
    if let Some(idx) = remark {
        variants.push(("remark", idx));
    }
    if let Some(idx) = remark_with_event {
        variants.push(("remark_with_event", idx));
    }

    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"meta");
    bytes.push(14);

    push_compact(&mut bytes, 4);
    push_call_variant_type(&mut bytes, 0, &variants);
    push_composite_type(&mut bytes, 1, &[("data", 2)]);
    push_composite_type(&mut bytes, 2, &[("free", 3)]);
    push_primitive_type(&mut bytes, 3, 7);

    push_compact(&mut bytes, 1);
    push_string(&mut bytes, "System");
    bytes.push(1);
    push_string(&mut bytes, "System");
    push_compact(&mut bytes, 1);
    push_string(&mut bytes, "Account");
    bytes.push(0);
    bytes.push(0);
    push_compact(&mut bytes, 1);
    push_compact(&mut bytes, 0);
    push_strings(&mut bytes, &[]);
    bytes.push(1);
    push_compact(&mut bytes, 0);
    bytes.push(0);
    push_compact(&mut bytes, 0);
    bytes.push(0);
    bytes.push(3);

    format!("0x{}", hex::encode(bytes))
}

fn genesis_hash() -> String {
    format!("0x{}", "11".repeat(32))
}

fn system_properties() -> Value {
    json!({
        "ss58Format": 42,
        "tokenSymbol": "TAO",
        "tokenDecimals": 9
    })
}

async fn read_request(ws: &mut TestWs, expected_method: &str) -> Value {
    let frame = ws
        .next()
        .await
        .expect("client must send request")
        .expect("request frame must be valid");
    let WsMessage::Text(text) = frame else {
        panic!("expected text request");
    };
    let req: Value = serde_json::from_str(text.as_ref()).expect("request must be JSON");
    assert_eq!(req["method"], expected_method);
    req
}

async fn send_response(ws: &mut TestWs, id: Value, result: Value) {
    let response = json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    });
    ws.send(WsMessage::Text(response.to_string().into()))
        .await
        .expect("response must send");
}

async fn serve_script(stream: tokio::net::TcpStream, script: RpcScript) {
    let mut ws = tokio_tungstenite::accept_async(stream)
        .await
        .expect("websocket handshake must succeed");
    for (method, result) in script {
        let req = read_request(&mut ws, method).await;
        send_response(&mut ws, req["id"].clone(), result).await;
    }
}

async fn scripted_rpc_server(scripts: Vec<RpcScript>) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test server must bind");
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        for script in scripts {
            let (stream, _) = listener.accept().await.expect("connection must arrive");
            serve_script(stream, script).await;
        }
    });
    (url, server)
}

fn wallet_dir(wallet_name: &str) -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("taolk")
        .join(wallet_name)
}

#[tokio::test]
async fn start_with_mirrors_uses_metadata_resolved_remark_calls() {
    let metadata = metadata_hex_with_system_calls(Some(4), Some(6));
    let scripts = vec![
        vec![
            ("chain_getBlockHash", json!(genesis_hash())),
            (
                "state_getRuntimeVersion",
                json!({"specVersion": 77, "transactionVersion": 88}),
            ),
            ("state_getMetadata", json!(metadata)),
            ("system_chain", json!("TestChain")),
            ("system_properties", system_properties()),
        ],
        vec![("system_properties", system_properties())],
        vec![("state_getStorage", Value::Null)],
        vec![("chain_subscribeNewHeads", json!("test-subscription"))],
    ];
    let (node_url, server) = scripted_rpc_server(scripts).await;
    let wallet_name = format!(
        "runtime-call-ids-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let wallet_dir = wallet_dir(&wallet_name);
    let _ = std::fs::remove_dir_all(&wallet_dir);
    let mirror_urls = vec!["http://127.0.0.1:1".to_string()];

    let result = taolk::session::Session::start_with_mirrors(
        &[0x42; 32],
        &node_url,
        &wallet_name,
        &mirror_urls,
        true,
    )
    .await;
    let _ = std::fs::remove_dir_all(wallet_dir);

    let (session, events) = result.expect("session startup must succeed");
    assert_eq!(session.chain_info.remark_calls.remark, Some((3, 4)));
    assert_eq!(session.chain_info.remark_calls.remark_with_event, (3, 6));
    assert!(session.has_mirror);

    let mut saw_chain_reader = false;
    let mut saw_mirror_reader = false;
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline && !(saw_chain_reader && saw_mirror_reader) {
        tokio::time::sleep(Duration::from_millis(10)).await;
        while let Ok(event) = events.try_recv() {
            match event {
                Event::ConnectionStatus(ConnState::Connected) => saw_chain_reader = true,
                Event::Status(status) if status == "Catching up..." => saw_mirror_reader = true,
                _ => {}
            }
        }
    }
    assert!(saw_chain_reader);
    assert!(saw_mirror_reader);

    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .expect("RPC server must finish")
        .expect("RPC server must not panic");
}

async fn submit_rpc_server() -> (String, tokio::task::JoinHandle<(u8, u8)>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test server must bind");
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("connection must arrive");
        let mut ws = tokio_tungstenite::accept_async(stream)
            .await
            .expect("websocket handshake must succeed");

        let req = read_request(&mut ws, "chain_getBlockHash").await;
        send_response(&mut ws, req["id"].clone(), json!(genesis_hash())).await;

        let req = read_request(&mut ws, "state_getRuntimeVersion").await;
        send_response(
            &mut ws,
            req["id"].clone(),
            json!({"specVersion": 77, "transactionVersion": 88}),
        )
        .await;

        let req = read_request(&mut ws, "system_accountNextIndex").await;
        send_response(&mut ws, req["id"].clone(), json!(0)).await;

        let req = read_request(&mut ws, "author_submitAndWatchExtrinsic").await;
        let ext_hex = req["params"][0]
            .as_str()
            .expect("submitted extrinsic must be hex");
        let ext_bytes = hex::decode(ext_hex.trim_start_matches("0x")).unwrap();
        let ext = samp::ExtrinsicBytes::from_bytes(ext_bytes);
        let call = samp::extrinsic::extract_call(&ext).expect("call must decode");
        send_response(&mut ws, req["id"].clone(), json!("test-submission")).await;

        let update = json!({
            "jsonrpc": "2.0",
            "method": "author_extrinsicUpdate",
            "params": {
                "result": {
                    "inBlock": format!("0x{}", "22".repeat(32))
                }
            }
        });
        ws.send(WsMessage::Text(update.to_string().into()))
            .await
            .expect("update must send");

        (call.pallet().get(), call.call().get())
    });
    (url, server)
}

#[tokio::test]
async fn submit_remark_preserves_metadata_resolved_call_ids_after_refresh() {
    let (node_url, server) = submit_rpc_server().await;
    let mut chain_info = common::test_chain_info();
    chain_info.remark_calls = RemarkCallIds {
        remark: Some((3, 4)),
        remark_with_event: (5, 6),
    };
    let signing = common::signing_from_seed(&common::ALICE_SEED);
    let remark = samp::RemarkBytes::from_bytes(b"runtime-call-id".to_vec());

    let block_hash =
        taolk::extrinsic::submit_remark(&node_url, &remark, &signing, "alice", &chain_info)
            .await
            .expect("submit must succeed");
    let call = tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .expect("RPC server must finish")
        .expect("RPC server must not panic");

    assert_eq!(block_hash, format!("0x{}", "22".repeat(32)));
    assert_eq!(call, (5, 6));
}
