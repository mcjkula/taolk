# CLI Reference

## Wallet

### Create

Create a new wallet with a fresh 12-word recovery phrase.

```
taolk wallet create --name <name> [--password <pw>]
```

If `--password` is omitted, taolk prompts interactively (with confirmation). The recovery phrase is displayed once -- write it down. There is no other way to recover the wallet.

Fails if a wallet with that name already exists.

### Import

Import a wallet from an existing recovery phrase or raw seed.

```
taolk wallet import --name <name> --mnemonic "word1 word2 ..." [--password <pw>]
taolk wallet import --name <name> --seed <64-hex-chars> [--password <pw>]
```

Exactly one of `--mnemonic` or `--seed` is required. Accepts 12 or 24 word BIP39 phrases.

### List

List all wallets and their file paths.

```
taolk wallet list
```

Wallets are stored in `~/.samp/wallets/`.

## Configuration

Configuration is stored as TOML at the platform config directory:

- macOS: `~/Library/Application Support/taolk/config.toml`
- Linux: `~/.config/taolk/config.toml`

Keys use dot-notation: `section.field`. Only explicitly set values are written to the file; everything else uses compiled defaults.

### Commands

**List all values** (shows which are user-set vs default):

```
taolk config list
```

**Get a single value** (prints raw value to stdout):

```
taolk config get <key>
```

Without a key, behaves like `list`.

**Set a value:**

```
taolk config set <key> <value>
```

For list fields (e.g., `network.mirrors`), pass multiple values:

```
taolk config set network.mirrors ws://mirror1.example.com ws://mirror2.example.com
```

**Remove a key** (reverts to default):

```
taolk config unset <key>
```

**Open in editor** (uses `$VISUAL` or `$EDITOR`):

```
taolk config edit
```

**Print config file path:**

```
taolk config path
```

### Typo correction

If you mistype a key name, taolk suggests the closest match (Levenshtein distance <= 3).

### Keys

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `wallet.default` | string | -- | Default wallet name |
| `network.node` | string | `wss://entrypoint-finney.opentensor.ai:443` | Subtensor node WebSocket URL |
| `network.mirrors` | string[] | -- | SAMP mirror URLs |
| `security.lock_timeout` | integer | `300` | Auto-lock timeout in seconds (0 disables) |
| `ui.sidebar_width` | integer | `28` | Sidebar width in columns |
| `ui.mouse` | boolean | `true` | Enable mouse support |
| `ui.timestamp_format` | string | `%H:%M` | Message time format (chrono strftime) |
| `ui.date_format` | string | `%Y-%m-%d %H:%M` | Full date format (chrono strftime) |

## Global flags

These flags apply when launching the TUI (no subcommand):

```
taolk [--wallet <name>] [--node <url>] [--mirror <url>]...
```

| Flag | Description |
|------|-------------|
| `--wallet <name>` | Select wallet (overrides `wallet.default`) |
| `--node <url>` | Subtensor node URL (overrides `network.node`) |
| `--mirror <url>` | SAMP mirror URL, repeatable (overrides `network.mirrors`) |

## Examples

Create a wallet and launch:

```
taolk wallet create --name alice
taolk
```

Import from mnemonic:

```
taolk wallet import --name bob --mnemonic "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
```

Point at a remote node with a mirror:

```
taolk --node ws://node.example.com:9944 --mirror https://mirror.example.com
```

Set the default wallet so you don't need `--wallet` every time:

```
taolk config set wallet.default alice
```

Disable auto-lock:

```
taolk config set security.lock_timeout 0
```

Use 24-hour timestamps with seconds:

```
taolk config set ui.timestamp_format "%H:%M:%S"
```
