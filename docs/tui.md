# TUI Guide

## Getting started

Install from source:

```
cargo install --path .
```

Create a wallet:

```
taolk wallet create --name alice
```

Launch the TUI:

```
taolk
```

Optional flags:

- `--wallet <name>` -- select wallet (overrides `wallet.default` config)
- `--node <url>` -- Subtensor node WebSocket URL
- `--mirror <url>` -- SAMP mirror URL (repeatable)

If no wallet is specified, taolk checks `wallet.default` in config, then auto-discovers. If no wallets exist, it exits with an error.

## Lock screen

The lock screen shows the taolk logo and a password prompt.

**Single wallet:** displays the wallet name. Press `i` or `Enter` to focus the password field. Type your password, press `Enter` to unlock.

**Multiple wallets:** displays a horizontal carousel. Use `Left`/`Right` (or `h`/`l`) to select a wallet. Press `i` or `Enter` to focus the password field.

**Timeout re-lock:** when the session locks due to inactivity (see `security.lock_timeout`), the lock screen shows only the current wallet -- no carousel, no wallet switching. Same password prompt.

**Manual lock:** `Ctrl+L` locks the session from within the TUI.

- `q` quits from the lock screen
- `Ctrl+C` quits from anywhere
- `Esc` exits the password field back to wallet selection (multi-wallet only)

Wrong passwords show an inline error. The password field clears on each failed attempt.

## Navigation

### Sidebar

The left sidebar lists all conversations grouped by type:

1. Inbox / Outbox
2. Threads (grouped by peer, sorted by most recent activity)
3. Channel Directory, then subscribed channels
4. Groups

Move between items: `j`/`k`, `Up`/`Down`, `Tab`/`Shift+Tab`.

Toggle sidebar visibility: `Space`.

Mouse: click a sidebar item to select it. Click the input area to enter Insert mode.

### Message scroll

Messages display bottom-up (newest at bottom). Scroll controls:

| Key | Action |
|-----|--------|
| `Ctrl+U` | Scroll up ~10 lines |
| `Ctrl+D` | Scroll down ~10 lines |
| `PageUp` | Scroll up ~20 lines |
| `PageDown` | Scroll down ~20 lines |
| `Home` | Jump to oldest messages |
| `G` or `End` | Jump to newest messages |

### Views

- **Inbox** -- incoming public and encrypted messages
- **Outbox** -- messages you sent (standalone public/encrypted)
- **Thread** -- two-party conversation with DAG-ordered messages
- **Channel** -- public broadcast channel, anyone can post
- **Group** -- encrypted multi-party conversation
- **Channel Directory** -- browse and subscribe to discovered channels

## Messaging

### Insert mode

Press `i` in a thread, channel, or group to enter Insert mode. The cursor appears in the input area.

Type your message. Editing keys:

| Key | Action |
|-----|--------|
| Characters | Insert text |
| `Backspace` | Delete before cursor |
| `Delete` | Delete after cursor |
| `Left`/`Right` | Move cursor |
| `Ctrl+Left`/`Ctrl+Right` | Jump by word |
| `Home`/`End` | Start/end of line |
| `Up`/`Down` | Move between lines (multiline) |
| `Ctrl+N` | Insert newline |

Press `Enter` to send. This builds the SAMP remark and enters Confirm mode, which shows the estimated transaction fee.

In Confirm mode:
- `Enter` submits the extrinsic
- `Esc` cancels and returns to editing

Press `Esc` in Insert mode to save the draft and return to Normal mode.

### Drafts

Drafts auto-save when you press `Esc` from Insert mode or switch conversations. Each thread, channel, and group has its own draft. Drafts restore when you return to the conversation and press `i`.

If you quit with unsaved drafts, taolk warns and requires a second `q` to confirm.

## Threads

### New thread

Press `n` in Normal mode to enter Compose mode. This shows a contact picker.

- Type to filter contacts by SS58 address
- `j`/`k` or `Up`/`Down` to move through the contact list
- `Enter` selects the contact and enters Insert mode
- Paste a full SS58 address directly

Write your message and press `Enter`. After confirming the fee, the thread is created on-chain.

### Replies

Within a thread, every message you send is a reply. The protocol references the latest message in the DAG automatically.

### Fetching gaps

Press `r` in a thread to fetch any missing messages from the chain. If a mirror is configured, channel views also fetch from the mirror.

## Standalone messages

Press `m` in Normal mode to send a standalone message (not part of a thread).

1. Select or enter an address (same contact picker as Compose)
2. Choose message type:
   - `p` -- public (plaintext on chain)
   - `e` -- encrypted (X25519 + ChaCha20Poly1305)
3. Write the message and confirm

Standalone messages appear in the recipient's Inbox and your Outbox.

## Channels

### Channel directory

Press `c` in Normal mode to open the Channel Directory. This lists all channels discovered from the chain.

- `Up`/`Down` to browse
- `Enter` to subscribe or unsubscribe (toggles)
- Type a `block:index` reference to subscribe to a channel by ID
- `c` (inside the directory) to create a new channel
- `Esc` to return

### Creating a channel

From the Channel Directory, press `c`:

1. Enter channel name, press `Enter`
2. Enter description, press `Enter`
3. Confirm the fee

The channel is created on-chain. You are automatically subscribed.

`Esc` during description steps back to the name input.

## Groups

Encrypted multi-party conversations. All members are fixed at creation time.

### Creating a group

Press `g` in Normal mode:

1. A member picker appears (you are always included)
2. Type to filter contacts, `Enter` to toggle a member, `Up`/`Down` to navigate
3. Paste SS58 addresses for contacts not yet known
4. `Tab` to finalize the member list (minimum: you + 1 other)
5. Write your first message and confirm

The group creation extrinsic includes the member list and first message. Each message is independently encrypted for every member.

### Group messages

Same as thread messaging: `i` to compose, `Enter` to send, `Esc` to save draft.

## Search

Press `/` in Normal mode to search within the current view. Type a query; results highlight as you type. Press `Enter` to keep the search active and return to Normal mode. Press `Esc` to clear the search.

## Copy a sender's SS58

Press `y` in Normal mode while viewing any chat to open the sender picker: a
list of unique senders from the current view, sorted by most recent activity.
Navigate with `↑`/`↓` (or `j`/`k`), press `Enter` to copy the highlighted
sender's full 48-character SS58 to the system clipboard, `Esc` to cancel. The
clipboard payload is delivered via OSC 52 escape sequence and works in
WezTerm, macOS Terminal.app, iTerm2, Alacritty, Kitty, and tmux (with
`set-clipboard on`); it also works over SSH because the escape is interpreted
on the client terminal.

You can also click directly on a sender's name in any rendered message to
copy their SS58 in one click — no picker required.

## Keyboard reference

### Normal mode

| Key | Action |
|-----|--------|
| `j` / `Down` / `Tab` | Next sidebar item |
| `k` / `Up` / `Shift+Tab` | Previous sidebar item |
| `i` | Enter Insert mode (thread/channel/group) |
| `n` | New thread (Compose mode) |
| `m` | Standalone message (Message mode) |
| `c` | Open Channel Directory |
| `g` | Create group |
| `r` | Fetch missing messages from chain |
| `/` | Search |
| `y` | Open sender picker (copy SS58 from chat) |
| `Space` | Toggle sidebar |
| `Ctrl+U` | Scroll up |
| `Ctrl+D` | Scroll down |
| `PageUp` / `PageDown` | Scroll up/down (large) |
| `Home` | Scroll to top |
| `G` / `End` | Scroll to bottom |
| `Ctrl+L` | Lock session |
| `q` | Quit (double-tap if drafts exist) |
| `Ctrl+C` | Quit immediately |

### Insert mode

| Key | Action |
|-----|--------|
| Characters | Insert text |
| `Backspace` / `Delete` | Delete |
| `Left` / `Right` | Move cursor |
| `Ctrl+Left` / `Ctrl+Right` | Move by word |
| `Home` / `End` | Start/end of input |
| `Up` / `Down` | Move between lines |
| `Ctrl+N` | Insert newline |
| `Enter` | Send (enter Confirm mode) |
| `Esc` | Save draft, return to Normal |

### Confirm mode

| Key | Action |
|-----|--------|
| `Enter` | Submit extrinsic |
| `Esc` | Cancel |

### Compose mode (new thread)

| Key | Action |
|-----|--------|
| Characters | Filter contacts |
| `j` / `k` / `Up` / `Down` | Navigate contact list |
| `Enter` | Select contact |
| `Backspace` | Clear filter |
| `Esc` | Clear filter or exit |

### Message mode (standalone)

Phase 1 (address): same as Compose mode.

Phase 2 (type selection):

| Key | Action |
|-----|--------|
| `p` | Public message |
| `e` | Encrypted message |
| `Esc` | Cancel |

### Channel Directory

| Key | Action |
|-----|--------|
| `Up` / `Down` | Browse channels |
| `Enter` | Subscribe/unsubscribe (or submit typed ref) |
| `c` | Create channel |
| `0-9` / `:` | Type channel block:index |
| `Backspace` | Clear typed ref |
| `Esc` | Clear input or exit directory |

### Create Channel

| Key | Action |
|-----|--------|
| Text input | Channel name (step 1) or description (step 2) |
| `Enter` | Advance to next step |
| `Esc` | Step back or cancel |

### Create Group Members

| Key | Action |
|-----|--------|
| Characters | Filter contacts |
| `Up` / `Down` | Navigate contact list |
| `Enter` | Toggle member / add address |
| `Tab` | Finalize members, enter Insert mode |
| `Esc` | Clear filter or cancel |

### Lock screen

| Key | Action |
|-----|--------|
| `i` / `Enter` | Focus password field |
| `Left` / `h` | Previous wallet (carousel) |
| `Right` / `l` | Next wallet (carousel) |
| `Esc` | Unfocus password field |
| `q` | Quit |
| `Ctrl+C` | Quit |
