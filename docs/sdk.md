# SDK Reference

## Installation

Add `taolk` as a library dependency. Disable the `tui` feature (enabled by default) to avoid pulling in terminal UI dependencies:

```toml
[dependencies]
taolk = { version = "2", default-features = false }
```

The `tui` feature gates `ratatui`, `crossterm`, `rpassword`, and `clap`. Without it, you get the core SDK: session management, wallet operations, SAMP encoding, chain submission.

## Hello World: Offline

Create a wallet, derive keys, and encode a SAMP public remark -- no chain connection needed.

```rust
use taolk::secret::{Password, Phrase, Seed};
use taolk::wallet;

fn main() {
    // Generate a fresh mnemonic and derive a seed
    let phrase = Phrase::generate();
    println!("Recovery phrase: {}", phrase.words().join(" "));

    let seed = Seed::from_phrase(&phrase);
    let signing_key = seed.derive_signing_key();
    let pubkey = signing_key.public_key();
    println!("Public key: {pubkey:?}");

    // Persist the seed in an encrypted wallet file
    let password = Password::new("hunter2".into());
    wallet::create("demo", &password, &seed).expect("create wallet");

    // Re-open the wallet to verify
    let recovered = wallet::open("demo", &password).expect("open wallet");
    assert!(seed.ct_eq(&recovered));
    println!("Wallet round-trip OK");
}
```

## Hello World: Connected

Open a wallet, start a session, send an encrypted message, and listen for events.

```rust
use taolk::secret::Password;
use taolk::{wallet, Event, Pubkey, Session};

#[tokio::main]
async fn main() {
    // Open the wallet
    let password = Password::new("hunter2".into());
    let seed = wallet::open("demo", &password).expect("open wallet");

    // Start a session (keep_seed = true so we can encrypt later)
    let (session, rx) = Session::start(
        seed.as_bytes(),
        "wss://entrypoint-finney.opentensor.ai:443",
        "demo",
        true,
    )
    .await
    .expect("start session");

    println!("Running as {}", session.ss58());

    // Send an encrypted message
    let recipient = Pubkey([0xab; 32]); // replace with real pubkey
    let body = taolk::MessageBody::from("hello");
    let remark = session
        .build_encrypted_message(seed.as_bytes(), &recipient, &body)
        .expect("build message");
    session.submit(&remark).await.expect("submit");

    // Listen for events
    while let Ok(event) = rx.recv() {
        match event {
            Event::MessageSent => println!("Message confirmed on-chain"),
            Event::NewMessage { decrypted_body: Some(body), sender, .. } => {
                println!("From {sender:?}: {body}");
            }
            Event::Error(e) => eprintln!("Error: {e}"),
            _ => {}
        }
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
    keep_seed: bool,
) -> Result<(Self, mpsc::Receiver<Event>)>
```

Derives an SR25519 keypair from the seed, connects to the subtensor node at `node_url`, opens the local database for `wallet_name`, loads persisted conversations, fetches the on-chain balance, and spawns a background task that subscribes to new blocks. When `keep_seed` is `true`, the session retains the seed in memory for encryption operations. Returns the session and a channel receiver for incoming events.

```rust
pub async fn start_with_mirrors(
    seed: &[u8; 32],
    node_url: &str,
    wallet_name: &str,
    mirror_urls: &[String],
    keep_seed: bool,
) -> Result<(Self, mpsc::Receiver<Event>)>
```

Same as `start`, but also spawns mirror sync tasks for each URL in `mirror_urls`. Mirrors provide off-chain indexing for channel history. Pass an empty slice to disable mirrors (equivalent to `start`).

### Sending messages

All `build_*` methods return `Result<RemarkBytes>` -- a SAMP-encoded remark ready for on-chain submission via `submit`. None of them touch the network; they only encode bytes. `RemarkBytes` is re-exported from the `samp` crate.

Encrypted methods require `seed: &[u8; 32]`. Unencrypted methods do not.

#### `build_public_message`

```rust
pub fn build_public_message(&self, recipient: &Pubkey, body: &MessageBody) -> Result<RemarkBytes>
```

Encodes a plaintext (unencrypted) message to `recipient`. The body is visible to anyone scanning the chain.

#### `build_encrypted_message`

```rust
pub fn build_encrypted_message(&self, seed: &[u8; 32], recipient: &Pubkey, body: &MessageBody) -> Result<RemarkBytes>
```

Encrypts `body` for `recipient` using Ristretto255 ECDH + ChaCha20-Poly1305. Returns `SdkError::Encryption` if the recipient public key is invalid.

#### `build_thread_root`

```rust
pub fn build_thread_root(&self, seed: &[u8; 32], recipient: &Pubkey, body: &MessageBody) -> Result<RemarkBytes>
```

Creates a new encrypted thread with `recipient`. The on-chain extrinsic that includes this remark becomes the thread's `BlockRef` anchor. Produces an encrypted thread SAMP envelope with all ref fields set to `BlockRef::ZERO` (no parent, no reply-to, no continues).

#### `build_thread_reply`

```rust
pub fn build_thread_reply(&self, seed: &[u8; 32], thread_idx: usize, body: &MessageBody) -> Result<RemarkBytes>
```

Appends an encrypted reply to the thread at index `thread_idx` in `session.threads`. Automatically fills in the thread ref, reply-to (last message in thread), and continues (last message sent by you). Returns `SdkError::NotFound` if `thread_idx` is out of bounds.

#### `build_channel_create`

```rust
pub fn build_channel_create(&self, name: &str, description: &str) -> Result<RemarkBytes>
```

Encodes a channel creation remark. `name` and `description` are plaintext and visible on-chain. The resulting extrinsic's `BlockRef` becomes the channel's permanent identifier.

#### `build_channel_message`

```rust
pub fn build_channel_message(&self, channel_idx: usize, body: &str) -> Result<RemarkBytes>
```

Encodes a plaintext message to the channel at index `channel_idx` in `session.channels`. Includes reply-to and continues refs for ordering. Returns `SdkError::NotFound` if the index is invalid.

#### `build_group_create`

```rust
pub fn build_group_create(&self, seed: &[u8; 32], members: &[Pubkey], body: &MessageBody) -> Result<RemarkBytes>
```

Creates an encrypted group with the given members. The body is the first message. Each member can decrypt independently. The member list is encoded into the encrypted payload.

#### `build_group_message`

```rust
pub fn build_group_message(&self, seed: &[u8; 32], group_idx: usize, body: &MessageBody) -> Result<RemarkBytes>
```

Sends an encrypted message to the group at index `group_idx` in `session.groups`. Each member can decrypt independently. Returns `SdkError::NotFound` if the index is invalid.

#### `submit`

```rust
pub async fn submit(&self, remark: &samp::RemarkBytes) -> Result<String>
```

Submits a `system.remark` extrinsic containing `remark` to the connected subtensor node. Returns the extrinsic hash as a hex string on success, or `SdkError::Chain` on failure. This is the only method that touches the network.

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
| `NewChannelMessage` | `sender: Pubkey`, `sender_ss58: String`, `channel_ref: BlockRef`, `body: String`, `reply_to: BlockRef`, `continues: BlockRef`, `block_number: u32`, `ext_index: u16`, `timestamp: u64` | Message in a subscribed channel. |
| `ChannelDiscovered` | `name: String`, `description: String`, `creator_ss58: String`, `channel_ref: BlockRef` | A channel creation was seen on-chain. |
| `GroupDiscovered` | `creator_pubkey: Pubkey`, `group_ref: BlockRef`, `members: Vec<Pubkey>` | A group was created that includes you. |
| `NewGroupMessage` | `sender: Pubkey`, `sender_ss58: String`, `group_ref: BlockRef`, `body: String`, `reply_to: BlockRef`, `continues: BlockRef`, `block_number: u32`, `ext_index: u16`, `timestamp: u64` | Message in a group you belong to. |
| `LockedOutbound` | `sender: Pubkey`, `block_number: u32`, `ext_index: u16`, `timestamp: u64`, `remark_bytes: Vec<u8>` | An outbound message was observed but could not be decrypted (seed not retained). |
| `MessageSent` | (none) | Confirmation that a submitted extrinsic was included in a block. |
| `BlockUpdate` | `u64` | New block number observed. |
| `FetchBlock` | `block_ref: BlockRef` | Request to fetch a specific block (gap fill). |
| `FetchChannelMirror` | `channel_ref: BlockRef` | Request to fetch channel history from a mirror. |
| `SubmitRemark` | `remark: Vec<u8>` | Internal: a remark needs to be submitted (used by mirror sync). |
| `GapsRefreshed` | (none) | Thread/channel gap detection completed. |
| `FeeEstimated` | `fee_display: String`, `fee_raw: Option<u128>` | Result of a fee estimation. |
| `BalanceUpdated` | `u128` | Account balance changed. |
| `ChainSnapshotRefreshed` | `info: String`, `token_symbol: String`, `token_decimals: u32` | Chain metadata refreshed (occurs on connect and reconnect). |
| `GenesisMismatch` | (none) | Connected node's genesis hash does not match the expected chain. |
| `ConnectionStatus` | `ConnState` | Connection state change. `ConnState::Connected` or `ConnState::Reconnecting { in_secs }`. |
| `Status` | `String` | Human-readable status message (e.g. "Connected to node"). |
| `Error` | `String` | Non-fatal error description. |
| `CatchupComplete` | (none) | Historical block scan finished; the session is fully synced. |

## Types

### Pubkey

```rust
pub use samp::Pubkey;
```

A 32-byte SR25519 public key, re-exported from the `samp` crate.

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
pub use samp::BlockRef;
```

On-chain reference: a block number and extrinsic index within that block, re-exported from the `samp` crate. Every thread, channel, group, and message is identified by the `BlockRef` of the extrinsic that created it.

- `BlockRef::ZERO` -- `{ block: 0, index: 0 }`. Used to indicate "no reference" (e.g. a thread root has no parent).
- `is_zero(&self) -> bool` -- returns `true` if both fields are zero.

### Secret types

All secret wrapper types live in the `taolk::secret` module. They use `zeroize` to clear memory on drop.

#### Seed

Wraps `Zeroizing<[u8; 32]>`.

| Method | Description |
|---|---|
| `from_bytes([u8; 32])` | Wrap raw bytes |
| `from_phrase(&Phrase)` | Derive from mnemonic |
| `from_hex(&str)` | Parse hex string |
| `as_bytes() -> &[u8; 32]` | Borrow inner bytes |
| `derive_signing_key() -> SigningKey` | Derive SR25519 keypair |
| `ct_eq(&Self) -> bool` | Constant-time equality |

#### Password

Wraps `Zeroizing<String>`.

| Method | Description |
|---|---|
| `new(String)` | Wrap a password string |
| `as_str() -> &str` | Borrow inner string |

#### Phrase

Wraps `Zeroizing<String>`.

| Method | Description |
|---|---|
| `generate()` | Random 12-word BIP-39 mnemonic |
| `parse(&str)` | Parse and validate a mnemonic |
| `words() -> Vec<&str>` | Split into word list |

#### SigningKey

Wraps a schnorrkel `Keypair`.

| Method | Description |
|---|---|
| `sign(&[u8]) -> [u8; 64]` | SR25519 signature |
| `public_key() -> Pubkey` | Extract public key |

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
pub fn open(name: &str, password: &Password) -> Result<Seed, WalletError>
```

Decrypts and returns the seed from `~/.samp/wallets/{name}.key`. Returns `WalletError::WrongPassword` if the password is incorrect, `WalletError::CorruptFile` if the file is malformed.

```rust
pub fn create(name: &str, password: &Password, seed: &Seed) -> Result<(), WalletError>
```

Encrypts `seed` and writes it to `~/.samp/wallets/{name}.key`. Creates the directory if needed.

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
| `security.require_password_per_send` | bool | `false` |
| `notifications.enabled` | bool | `true` |
| `notifications.volume` | u8 | `100` |
| `notifications.dm` | bool | `true` |
| `notifications.ambient` | bool | `false` |
| `notifications.mention` | bool | `true` |
| `ui.sidebar_width` | u16 | `28` |
| `ui.mouse` | bool | `true` |
| `ui.timestamp_format` | string | `%H:%M` |
| `ui.date_format` | string | `%Y-%m-%d %H:%M` |
