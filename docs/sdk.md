# SDK Reference

## Installation

Add `taolk` as a library dependency. Disable the `tui` feature (enabled by default) to avoid pulling in terminal UI dependencies:

```toml
[dependencies]
taolk = { path = "../taolk", default-features = false }
```

The `tui` feature gates `ratatui`, `crossterm`, `rpassword`, and `clap`. Without it, you get the core SDK: session management, wallet operations, SAMP encoding, chain submission.

## Quick start

```rust
use taolk::{wallet, Session};

#[tokio::main]
async fn main() {
    let seed = wallet::open("default", "hunter2").unwrap();
    let (session, rx) = Session::start(&seed, "wss://entrypoint-finney.opentensor.ai:443", "default")
        .await
        .unwrap();

    // Send an encrypted message
    let recipient = /* Pubkey from SS58 or bytes */;
    let remark = session.build_encrypted_message(&recipient, "hello").unwrap();
    session.submit(&remark).await.unwrap();

    // Receive events
    while let Ok(event) = rx.recv() {
        println!("{event:?}");
    }
}
```

## Session

The `Session` struct holds the keypair, chain connection state, and all conversation data (threads, channels, groups, inbox, outbox). It also holds a local SQLite database handle for persistence.

### Creating a session

```rust
pub async fn start(
    seed: &[u8; 32],
    node_url: &str,
    wallet_name: &str,
) -> Result<(Self, mpsc::Receiver<Event>)>
```

Derives an SR25519 keypair from the seed, connects to the subtensor node at `node_url`, opens the local database for `wallet_name`, loads persisted conversations, fetches the on-chain balance, and spawns a background task that subscribes to new blocks. Returns the session and a channel receiver for incoming events.

```rust
pub async fn start_with_mirrors(
    seed: &[u8; 32],
    node_url: &str,
    wallet_name: &str,
    mirror_urls: &[String],
) -> Result<(Self, mpsc::Receiver<Event>)>
```

Same as `start`, but also spawns mirror sync tasks for each URL in `mirror_urls`. Mirrors provide off-chain indexing for channel history. Pass an empty slice to disable mirrors (equivalent to `start`).

### Sending messages

All `build_*` methods return `Result<Vec<u8>>` -- a SAMP-encoded remark ready for on-chain submission via `submit`. None of them touch the network; they only encode bytes.

#### `build_public_message`

```rust
pub fn build_public_message(&self, recipient: &Pubkey, body: &str) -> Result<Vec<u8>>
```

Encodes a plaintext (unencrypted) message to `recipient`. The body is visible to anyone scanning the chain.

#### `build_encrypted_message`

```rust
pub fn build_encrypted_message(&self, recipient: &Pubkey, body: &str) -> Result<Vec<u8>>
```

Encrypts `body` for `recipient` using X25519 ECDH with a random 12-byte nonce. Produces a `0x11` (encrypted direct) SAMP envelope. Returns `SdkError::Encryption` if the recipient public key is invalid.

#### `build_thread_root`

```rust
pub fn build_thread_root(&self, recipient: &Pubkey, body: &str) -> Result<Vec<u8>>
```

Creates a new encrypted thread with `recipient`. The on-chain extrinsic that includes this remark becomes the thread's `BlockRef` anchor. Produces a `0x12` (encrypted thread) SAMP envelope with all ref fields set to `BlockRef::ZERO` (no parent, no reply-to, no continues).

#### `build_thread_reply`

```rust
pub fn build_thread_reply(&self, thread_idx: usize, body: &str) -> Result<Vec<u8>>
```

Appends an encrypted reply to the thread at index `thread_idx` in `session.threads`. Automatically fills in the thread ref, reply-to (last message in thread), and continues (last message sent by you). Returns `SdkError::NotFound` if `thread_idx` is out of bounds.

#### `build_channel_create`

```rust
pub fn build_channel_create(&self, name: &str, description: &str) -> Result<Vec<u8>>
```

Encodes a channel creation remark. `name` and `description` are plaintext and visible on-chain. The resulting extrinsic's `BlockRef` becomes the channel's permanent identifier.

#### `build_channel_message`

```rust
pub fn build_channel_message(&self, channel_idx: usize, body: &str) -> Result<Vec<u8>>
```

Encodes a plaintext message to the channel at index `channel_idx` in `session.channels`. Includes reply-to and continues refs for ordering. Returns `SdkError::NotFound` if the index is invalid.

#### `build_group_create`

```rust
pub fn build_group_create(&self, members: &[Pubkey], body: &str) -> Result<Vec<u8>>
```

Creates an encrypted group with the given members. The body is the first message. Each member can decrypt independently. The member list is encoded into the encrypted payload.

#### `build_group_message`

```rust
pub fn build_group_message(&self, group_idx: usize, body: &str) -> Result<Vec<u8>>
```

Sends an encrypted message to the group at index `group_idx` in `session.groups`. Each member can decrypt independently. Returns `SdkError::NotFound` if the index is invalid.

#### `submit`

```rust
pub async fn submit(&self, remark: &[u8]) -> Result<String>
```

Submits a `system.remark` extrinsic containing `remark` to the connected subtensor node. Returns the extrinsic hash as a hex string on success, or `SdkError::Chain` on failure. This is the only method that touches the network.

You can also use `submit` with arbitrary bytes if you encode your own SAMP remarks.

### Queries

```rust
pub async fn fetch_balance(&self) -> Result<u128>
```

Queries the node for the account's free balance (in plancks). Returns `SdkError::Chain` on RPC failure.

```rust
pub async fn estimate_fee(&self, remark: &[u8]) -> Result<u128>
```

Estimates the transaction fee for submitting `remark` as a `system.remark` extrinsic. Returns the fee in plancks.

### Accessors

```rust
pub fn pubkey(&self) -> Pubkey          // Your 32-byte SR25519 public key
pub fn ss58(&self) -> &str              // Your SS58-encoded address
```

Conversation state is stored in public fields:

- `session.threads: Vec<Thread>` -- encrypted 1:1 conversations
- `session.channels: Vec<Channel>` -- subscribed public channels
- `session.groups: Vec<Group>` -- encrypted group conversations
- `session.inbox: Vec<InboxMessage>` -- received direct messages (non-thread)
- `session.outbox: Vec<InboxMessage>` -- sent direct messages (non-thread)
- `session.known_channels: Vec<ChannelInfo>` -- discovered but not necessarily subscribed channels
- `session.balance: Option<u128>` -- last known balance, if fetched
- `session.token_symbol: String` -- chain token symbol (e.g. "TAO")
- `session.token_decimals: u32` -- chain token decimal places

## Events

```rust
pub enum Event
```

Events arrive on the `mpsc::Receiver<Event>` returned by `Session::start`. All `timestamp` fields are Unix seconds (block timestamp milliseconds divided by 1000).

| Variant | Fields | Description |
|---|---|---|
| `NewMessage` | `sender: Pubkey`, `content_type: u8`, `recipient: Pubkey`, `decrypted_body: Option<String>`, `thread_ref: BlockRef`, `reply_to: BlockRef`, `continues: BlockRef`, `block_number: u32`, `ext_index: u16`, `timestamp: u64` | Incoming direct or thread message. `decrypted_body` is `None` if decryption failed or the message was not for you. `thread_ref` is `BlockRef::ZERO` for non-thread messages. |
| `NewChannelMessage` | `sender_ss58: String`, `channel_ref: BlockRef`, `body: String`, `reply_to: BlockRef`, `continues: BlockRef`, `block_number: u32`, `ext_index: u16`, `timestamp: u64` | Message in a subscribed channel. |
| `ChannelDiscovered` | `name: String`, `description: String`, `creator_ss58: String`, `channel_ref: BlockRef` | A channel creation was seen on-chain. |
| `GroupDiscovered` | `creator_pubkey: Pubkey`, `group_ref: BlockRef`, `members: Vec<Pubkey>` | A group was created that includes you. |
| `NewGroupMessage` | `sender_ss58: String`, `group_ref: BlockRef`, `body: String`, `reply_to: BlockRef`, `continues: BlockRef`, `block_number: u32`, `ext_index: u16`, `timestamp: u64` | Message in a group you belong to. |
| `MessageSent` | (none) | Confirmation that a submitted extrinsic was included in a block. |
| `BlockUpdate` | `u64` | New block number observed. |
| `FetchBlock` | `block_ref: BlockRef` | Request to fetch a specific block (gap fill). |
| `FetchChannelMirror` | `channel_ref: BlockRef` | Request to fetch channel history from a mirror. |
| `SubmitRemark` | `remark: Vec<u8>` | Internal: a remark needs to be submitted (used by mirror sync). |
| `GapsRefreshed` | (none) | Thread/channel gap detection completed. |
| `FeeEstimated` | `fee_display: String`, `fee_raw: Option<u128>` | Result of a fee estimation. |
| `BalanceUpdated` | `u128` | Account balance changed. |
| `Status` | `String` | Human-readable status message (e.g. "Connected to node"). |
| `Error` | `String` | Non-fatal error description. |

## Types

### Pubkey

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Pubkey(pub [u8; 32]);
```

A 32-byte SR25519 public key. The inner `[u8; 32]` is public.

- `Pubkey::ZERO` -- all-zero constant, used as a sentinel.
- `Deref<Target = [u8; 32]>` -- you can use a `Pubkey` anywhere a `&[u8; 32]` is expected.
- `From<[u8; 32]>` and `Into<[u8; 32]>` -- convert freely between raw bytes and `Pubkey`.

Construct from bytes:

```rust
let pk = Pubkey([0xab; 32]);
let pk2 = Pubkey::from(some_bytes);
```

### BlockRef

```rust
pub struct BlockRef {
    pub block: u32,
    pub index: u16,
}
```

On-chain reference: a block number and extrinsic index within that block. Every thread, channel, group, and message is identified by the `BlockRef` of the extrinsic that created it.

- `BlockRef::ZERO` -- `{ block: 0, index: 0 }`. Used to indicate "no reference" (e.g. a thread root has no parent).
- `is_zero(&self) -> bool` -- returns `true` if both fields are zero.

### SdkError

```rust
pub enum SdkError {
    Encryption(String),
    Decryption(String),
    InvalidAddress(String),
    Chain(String),
    NotFound(String),
    Database(String),
    Wallet(String),
    Other(String),
}
```

All variants carry a `String` message. The SDK uses `type Result<T> = std::result::Result<T, SdkError>`.

| Variant | When it occurs |
|---|---|
| `Encryption` | `build_encrypted_message`, `build_thread_root`, `build_thread_reply`, `build_group_create`, `build_group_message` -- invalid recipient key or internal crypto failure. |
| `Decryption` | Decryption of an incoming message failed. |
| `InvalidAddress` | An SS58 address could not be parsed. |
| `Chain` | RPC call to the subtensor node failed (`submit`, `fetch_balance`, `estimate_fee`, `start`). |
| `NotFound` | Index out of bounds for `build_thread_reply`, `build_channel_message`, `build_group_message`. |
| `Database` | SQLite error during `start` or persistence operations. |
| `Wallet` | Invalid seed bytes passed to `start` or `start_with_mirrors`. |
| `Other` | Everything else (e.g. `build_channel_create` encoding failure). |

## Wallet

Wallet files are stored at `~/.samp/wallets/{name}.key`. Each file is 93 bytes: a version byte, 32-byte Argon2id salt, 12-byte nonce, and 48-byte ChaCha20-Poly1305 ciphertext (encrypting the 32-byte seed). File permissions are set to `0600` on Unix.

### Opening and creating wallets

```rust
pub fn open(name: &str, password: &str) -> Result<Zeroizing<[u8; 32]>, WalletError>
```

Decrypts and returns the seed from `~/.samp/wallets/{name}.key`. Returns `WalletError::WrongPassword` if the password is incorrect, `WalletError::CorruptFile` if the file is malformed.

```rust
pub fn create(name: &str, password: &str, seed: &[u8; 32]) -> Result<(), WalletError>
```

Encrypts `seed` and writes it to `~/.samp/wallets/{name}.key`. Creates the directory if needed.

```rust
pub fn open_at(path: &Path, password: &str) -> Result<Zeroizing<[u8; 32]>, WalletError>
pub fn create_at(path: &Path, password: &str, seed: &[u8; 32]) -> Result<(), WalletError>
```

Same as `open`/`create` but at an arbitrary filesystem path.

### Seed derivation

```rust
pub fn generate_mnemonic() -> Mnemonic
```

Generates a random 12-word BIP-39 mnemonic from 128 bits of OS entropy.

```rust
pub fn seed_from_mnemonic(mnemonic: &Mnemonic) -> [u8; 32]
```

Derives a 32-byte seed from a BIP-39 mnemonic using PBKDF2-HMAC-SHA512 (2048 rounds, salt `b"mnemonic"`). Takes the first 32 bytes of the 64-byte output. This matches Substrate's mini-secret derivation.

```rust
pub fn seed_from_hex(hex_str: &str) -> Result<[u8; 32], String>
```

Parses a hex string (with or without `0x` prefix) into a 32-byte seed. Returns an error if the input is not exactly 64 hex characters.

## Configuration

Config is stored as TOML at the platform config directory (e.g. `~/.config/taolk/config.toml` on Linux, `~/Library/Application Support/taolk/config.toml` on macOS).

```rust
pub fn load() -> Config
```

Reads and parses the config file. Returns `Config::default()` if the file does not exist or cannot be parsed.

```rust
pub fn get_value(config: &Config, key: &str) -> String
```

Returns the current value of a dotted key (e.g. `"network.node"`, `"ui.mouse"`) as a display string.

```rust
pub fn set_key(key: &str, raw: &[String]) -> Result<String, String>
```

Validates and writes a single key to the config file using read-modify-write. `raw` is the value split by whitespace (for list values like `network.mirrors`, each element becomes an array entry). Returns the new display value on success.

Available keys:

| Key | Type | Default |
|---|---|---|
| `wallet.default` | string | (none) |
| `network.node` | string | `wss://entrypoint-finney.opentensor.ai:443` |
| `network.mirrors` | string[] | (none) |
| `security.lock_timeout` | u64 | `300` |
| `ui.sidebar_width` | u16 | `28` |
| `ui.mouse` | bool | `true` |
| `ui.timestamp_format` | string | `%H:%M` |
| `ui.date_format` | string | `%Y-%m-%d %H:%M` |

## Example: Echo bot

A bot that listens for encrypted thread messages and replies with the same text:

```rust
use taolk::{wallet, Session, Event, Pubkey, BlockRef};

#[tokio::main]
async fn main() {
    let seed = wallet::open("bot", "password").expect("open wallet");
    let (mut session, rx) = Session::start(&seed, "wss://entrypoint-finney.opentensor.ai:443", "bot")
        .await
        .expect("start session");

    println!("Bot running as {}", session.ss58());

    loop {
        let event = match rx.recv() {
            Ok(e) => e,
            Err(_) => break,
        };

        match event {
            Event::NewMessage {
                sender,
                decrypted_body: Some(body),
                thread_ref,
                block_number,
                ext_index,
                timestamp,
                ..
            } => {
                // Ingest the message so session state is up to date
                let msg_ref = BlockRef { block: block_number, index: ext_index };
                let new_msg = taolk::conversation::NewMessage {
                    sender_ss58: taolk::util::ss58_short(&sender),
                    timestamp: chrono::DateTime::from_timestamp(timestamp as i64, 0)
                        .unwrap_or_default(),
                    body: body.clone(),
                    reply_to: BlockRef::ZERO,
                    continues: BlockRef::ZERO,
                    block_number,
                    ext_index,
                };
                session.add_thread_message(sender, session.pubkey(), thread_ref, new_msg);

                // Find the thread and reply
                let thread_idx = session.threads.len() - 1;
                let remark = session.build_thread_reply(thread_idx, &body)
                    .expect("build reply");
                session.submit(&remark).await.expect("submit");
            }
            Event::Error(e) => eprintln!("error: {e}"),
            _ => {}
        }
    }
}
```
