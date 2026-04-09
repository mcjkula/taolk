# Changelog

## v2.0.0 — UI/UX redesign

Full TUI redesign around six architectural pillars: theme + chrome foundation,
focus-based keyboard model, lean in-tree text buffer, chat list + timeline
theme-wiring, welcome + hintbar + minimum-size contract, and a slash-command
palette with fuzzy jump.

### Added
- **Theme system** — `Theme` struct with 8 color slots (`border`,
  `border_focus`, `text`, `text_dim`, `text_strong`, `accent`, `timestamp`,
  `sender_rotation`) and 6 ship-ready themes: Mocha (default), Latte, Tokyo
  Night, Gruvbox Dark, Rose Pine, Monochrome. TrueColor / 256-color (6×6×6
  cube quantization) / Mono rendering modes selected by explicit config —
  no runtime capability detection.
- **`chrome::panel`** — single source of truth for pane chrome. Rounded
  borders, 1-char padding, focus-sensitive accent highlight. Every pane
  uses it.
- **Focus model** — `Focus { Composer, Timeline }` replaces the 11-variant
  `Mode` enum. `Overlay` enum layers on top (Help, Confirm, Compose, Message,
  CreateChannel, CreateChannelDesc, CreateGroupMembers, Search, SenderPicker,
  CommandPalette, FuzzyJump). Composer is **focused by default** when a
  conversation is opened — no more `i`-to-type friction.
- **`TextBuffer`** — lean in-tree text editor (`src/ui/composer.rs`) with
  UTF-8-safe cursor, word jumps over `CharKind::{Space, Punct, Other}`,
  multi-line navigation, edit-key dispatch. Zero external deps.
- **Welcome screen** — centered ASCII banner + SAMP tagline + user's SS58
  + shortcut list, rendered when the inbox is empty.
- **Hint bar** — persistent, context-sensitive 5–8-key hints at the
  bottom, driven by `(focus, overlay)`. Status messages override hints
  when active.
- **Minimum terminal size** — typed refusal at 100×28 with exact
  required/current dimensions.
- **Slash command registry** — `src/cmd/registry.rs`. 10 executable
  commands: `/help`, `/quit`, `/theme <name>`, `/sidebar`, `/search`,
  `/new`, `/message`, `/channels`, `/inbox`, `/outbox`.
- **Command palette** (`Ctrl+P`) — centered popup with nucleo-matcher
  fuzzy ranking over the command registry.
- **Fuzzy jump** (`Ctrl+J`) — same popup pattern over conversations
  (Inbox, Sent, Channels, threads, subscribed channels, groups).
- **Mouse scroll wheel** — scrolls the timeline; left-click on timeline
  area focuses it; clicks into the composer area focus the composer.
  Gated by `ui.mouse` config.
- **Config fields** — `ui.theme`, `ui.icons`, `ui.colors` (all enum,
  kebab-case serialized).
- **`scripts/ui_tripwire.sh`** — prevents regressions on the
  architectural commitments (no `Mode::`, no `cursor_pos` on `App`,
  `Borders::ALL` only in chrome/modal, theme-wired files stay clean).

### Changed
- `src/ui/sidebar.rs` → `chat_list.rs`, fully theme-wired.
- `src/ui/messages.rs` → `timeline.rs`, sender color rotation sourced
  from `theme.sender_rotation`, timestamp + sender name headlines
  theme-wired.
- `src/ui/status.rs` → `statusline.rs`; mode pill removed (hintbar
  supersedes it).
- `App::input: String` + `cursor_pos: usize` → `input: TextBuffer`.
  `handle_text_input` shrinks from 40 lines to 5.
- `Overlay` variants now side-carry state via `app.palette: Option<PaletteState>` /
  `app.jump: Option<JumpState>`, keeping the enum `Copy`-compatible.

### Deferred to 2.0.1
- Inline `@` / `#` assist popups at cursor position.
- Full List+ListState+ListItem cache rewrite of timeline.
- `overlay/which_key.rs` leader popup (requires overlay stacking).
- `overlay/confirm.rs` typed state rewrite — `Overlay::Confirm` still
  reads from the legacy `pending_*` fields on `App`.
- Sidebar selection via `ListState` (currently stateless `List`).

### Removed
- The `Mode` enum and all 11 variants.
- The `handle_text_input` `Left`/`Right`/`Backspace`/`Delete`/`Home`/`End`
  logic (moved into `TextBuffer::handle_edit_key`).
- `const SENDER_COLORS` hardcoded array (replaced by `theme.sender_rotation`).
- Bottom-bar mode pill (the hintbar replaces it).

### Dependencies
- Added `nucleo-matcher = "0.3"` (fuzzy matching for palette + jump).
- **Not** added: `tui-textarea`. Kept our own `TextBuffer` — fewer than
  300 lines, no version pin, fully audited.

### Commits (release/2.0.0)
- `e5817d1` — theme/chrome/symbols foundation
- `196e344` — Mode → Focus + Overlay, composer focused by default
- `77af4c0` — TextBuffer composer
- `1e5e82f` — chat_list/timeline + theme slot wiring
- `3ba7bd4` — hintbar/welcome/min-size, status → statusline
- `c04ed24` — command registry, palette, fuzzy jump, overlay module
- this commit — mouse polish, tripwire, changelog
