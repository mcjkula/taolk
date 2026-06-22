//! UI glyphs as a swappable theme.
//!
//! `IconSet` holds one glyph per UI affordance; three const instances cover the
//! cases: `NERD` (Nerd Font Material Design glyphs, needs a patched font),
//! `UNICODE` (plain single-cell symbols that render anywhere) and `ASCII`
//! (pure ASCII for the most limited terminals). The active set is chosen once at
//! startup (see `icons()`/`init()`); rendering reads it through `icons()`.
//!
//! Every glyph in every set must be exactly one display cell wide and a single
//! Unicode scalar — the tests below enforce it. The `NERD` set targets Nerd
//! Fonts v3 (`nf-md-*`, U+F0001+); the recommended fallback font is
//! "Symbols Nerd Font Mono" (the Mono variant keeps glyphs single-cell).

// `resolve`/`init` are consumed by the startup commit; until then they are
// intentionally unused. Removed once startup wires them in.
#![allow(dead_code)]

/// Nerd Font Material Design glyphs (v3 nf-md-*). Needs a patched font.
pub const NERD: IconSet = IconSet {
    inbox: "\u{F02FB}",
    outbox: "\u{F048A}",
    threads: "\u{F0369}",
    channels: "\u{F0423}",
    groups: "\u{F0849}",
    public: "\u{F0FC6}",
    encrypted: "\u{F033E}",
    creator: "\u{F01A5}",
    draft: "\u{F03EB}",
    check: "\u{F012C}",
    error: "\u{F0028}",
    lock_clock: "\u{F097F}",
    history: "\u{F02DA}",
    sync: "\u{F04E6}",
    block: "\u{F01A7}",
    account: "\u{F0B55}",
    wallet: "\u{F0BDD}",
    key: "\u{F030B}",
    help: "\u{F0625}",
    exit: "\u{F0206}",
    magnify: "\u{F0349}",
    menu: "\u{F035C}",
    keyboard: "\u{F097B}",
    cog: "\u{F0493}",
    refresh: "\u{F0450}",
    swap: "\u{F04E1}",
    copy: "\u{F018F}",
    lock_open: "\u{F0340}",
    chevron_left: "\u{F0141}",
    chevron_right: "\u{F0142}",
    arrow_up: "\u{F005D}",
    arrow_down: "\u{F0045}",
    arrow_left: "\u{F004D}",
    arrow_right: "\u{F0054}",
};

/// Plain single-cell Unicode symbols (East-Asian-Width N) that render on any
/// terminal without a Nerd Font. The default theme.
pub const UNICODE: IconSet = IconSet {
    inbox: "\u{21A7}",   // downwards arrow from bar
    outbox: "\u{21A5}",  // upwards arrow from bar
    threads: "\u{2263}", // strictly equivalent to (stacked bars)
    channels: "#",
    groups: "\u{2042}",     // asterism
    public: "\u{2641}",     // earth
    encrypted: "\u{2327}",  // x in a rectangle box
    creator: "\u{2734}",    // eight pointed star
    draft: "\u{270E}",      // lower right pencil
    check: "\u{2713}",      // check mark
    error: "\u{203C}",      // double exclamation
    lock_clock: "\u{29D7}", // black hourglass
    history: "\u{21BA}",    // anticlockwise circle arrow
    sync: "\u{21BB}",       // clockwise circle arrow
    block: "\u{25AA}",      // black small square
    account: "\u{263A}",    // white smiling face
    wallet: "\u{25AD}",     // white rectangle
    key: "\u{2325}",        // option key
    help: "?",
    exit: "\u{2715}",          // multiplication x
    magnify: "\u{2315}",       // telephone recorder (lens)
    menu: "\u{2263}",          // stacked bars
    keyboard: "\u{2328}",      // keyboard
    cog: "\u{2699}",           // gear
    refresh: "\u{21BB}",       // clockwise circle arrow
    swap: "\u{21C4}",          // rightwards over leftwards arrow
    copy: "\u{29C9}",          // two joined squares
    lock_open: "\u{25CC}",     // dotted circle
    chevron_left: "\u{2039}",  // single left angle quote
    chevron_right: "\u{203A}", // single right angle quote
    arrow_up: "\u{25B4}",      // small up triangle
    arrow_down: "\u{25BE}",    // small down triangle
    arrow_left: "\u{25C2}",    // small left triangle
    arrow_right: "\u{25B8}",   // small right triangle
};

/// Pure ASCII for the most limited terminals (and non-TTY output).
pub const ASCII: IconSet = IconSet {
    inbox: "i",
    outbox: "o",
    threads: "t",
    channels: "#",
    groups: "g",
    public: "p",
    encrypted: "e",
    creator: "@",
    draft: "d",
    check: "+",
    error: "!",
    lock_clock: "T",
    history: "h",
    sync: "s",
    block: "#",
    account: "a",
    wallet: "w",
    key: "k",
    help: "?",
    exit: "q",
    magnify: "/",
    menu: "=",
    keyboard: "K",
    cog: "*",
    refresh: "r",
    swap: "~",
    copy: "c",
    lock_open: "u",
    chevron_left: "<",
    chevron_right: ">",
    arrow_up: "^",
    arrow_down: "v",
    arrow_left: "<",
    arrow_right: ">",
};

/// One glyph per UI affordance. Fields are `&'static str` so a set is a plain
/// const and call sites keep `&'static str` types.
pub struct IconSet {
    pub inbox: &'static str,
    pub outbox: &'static str,
    pub threads: &'static str,
    pub channels: &'static str,
    pub groups: &'static str,
    pub public: &'static str,
    pub encrypted: &'static str,
    pub creator: &'static str,
    pub draft: &'static str,
    pub check: &'static str,
    pub error: &'static str,
    pub lock_clock: &'static str,
    pub history: &'static str,
    pub sync: &'static str,
    pub block: &'static str,
    pub account: &'static str,
    pub wallet: &'static str,
    pub key: &'static str,
    pub help: &'static str,
    pub exit: &'static str,
    pub magnify: &'static str,
    pub menu: &'static str,
    pub keyboard: &'static str,
    pub cog: &'static str,
    pub refresh: &'static str,
    pub swap: &'static str,
    pub copy: &'static str,
    pub lock_open: &'static str,
    pub chevron_left: &'static str,
    pub chevron_right: &'static str,
    pub arrow_up: &'static str,
    pub arrow_down: &'static str,
    pub arrow_left: &'static str,
    pub arrow_right: &'static str,
}

use std::sync::OnceLock;

/// The three selectable themes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IconStyle {
    Nerd,
    Unicode,
    Ascii,
}

impl IconStyle {
    /// Parse a config/env value; `None` for anything unrecognised.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "nerd" => Some(Self::Nerd),
            "unicode" => Some(Self::Unicode),
            "ascii" => Some(Self::Ascii),
            _ => None,
        }
    }

    /// The theme this style selects.
    pub fn set(self) -> &'static IconSet {
        match self {
            Self::Nerd => &NERD,
            Self::Unicode => &UNICODE,
            Self::Ascii => &ASCII,
        }
    }
}

static ACTIVE: OnceLock<&'static IconSet> = OnceLock::new();

/// The active theme. Defaults to the safe `UNICODE` set until `init` runs.
pub fn icons() -> &'static IconSet {
    ACTIVE.get().copied().unwrap_or(&UNICODE)
}

/// Set the active theme once. The first call wins; later calls (e.g. a wallet
/// switch re-entering the session) are no-ops.
pub fn init(set: &'static IconSet) {
    let _ = ACTIVE.set(set);
}

/// Pure theme resolution. Precedence: `TAOLK_ICONS` env, then `NERD_FONT=1`,
/// then the `ui.icons` config value, then a non-TTY downgrade to ASCII, then the
/// default `UNICODE`. An explicit env/config choice beats the non-TTY rule.
pub fn resolve_with(
    cfg_value: &str,
    taolk_env: Option<&str>,
    nerd_env: Option<&str>,
    is_tty: bool,
) -> &'static IconSet {
    if let Some(style) = taolk_env.and_then(IconStyle::parse) {
        return style.set();
    }
    if nerd_env == Some("1") {
        return &NERD;
    }
    if let Some(style) = IconStyle::parse(cfg_value) {
        return style.set();
    }
    if !is_tty {
        return &ASCII;
    }
    &UNICODE
}

/// Resolve the active theme from the real environment and TTY state.
pub fn resolve(cfg_value: &str) -> &'static IconSet {
    use std::io::IsTerminal;
    let taolk = std::env::var("TAOLK_ICONS").ok();
    let nerd = std::env::var("NERD_FONT").ok();
    resolve_with(
        cfg_value,
        taolk.as_deref(),
        nerd.as_deref(),
        std::io::stdout().is_terminal(),
    )
}

/// An icon referenced from a `const` table, where a runtime `&str` can't go.
/// Resolved to a glyph through the active theme at render time.
#[derive(Clone, Copy)]
pub enum Icon {
    Threads,
    Outbox,
    Groups,
    Magnify,
    Channels,
    Inbox,
    Menu,
    Help,
    Block,
    Refresh,
    Copy,
    LockOpen,
    Encrypted,
    Swap,
    Exit,
    Keyboard,
    Cog,
    Draft,
    Check,
    Account,
}

impl Icon {
    /// The glyph for this icon in a given theme (pure).
    pub fn pick(self, set: &IconSet) -> &'static str {
        match self {
            Icon::Threads => set.threads,
            Icon::Outbox => set.outbox,
            Icon::Groups => set.groups,
            Icon::Magnify => set.magnify,
            Icon::Channels => set.channels,
            Icon::Inbox => set.inbox,
            Icon::Menu => set.menu,
            Icon::Help => set.help,
            Icon::Block => set.block,
            Icon::Refresh => set.refresh,
            Icon::Copy => set.copy,
            Icon::LockOpen => set.lock_open,
            Icon::Encrypted => set.encrypted,
            Icon::Swap => set.swap,
            Icon::Exit => set.exit,
            Icon::Keyboard => set.keyboard,
            Icon::Cog => set.cog,
            Icon::Draft => set.draft,
            Icon::Check => set.check,
            Icon::Account => set.account,
        }
    }

    /// The glyph for this icon in the active theme.
    pub fn glyph(self) -> &'static str {
        self.pick(icons())
    }
}

#[cfg(test)]
impl IconSet {
    /// All themes, for tests that must check every set.
    pub const ALL: [&'static IconSet; 3] = [&NERD, &UNICODE, &ASCII];

    /// (field name, glyph) for every field, so tests can iterate exhaustively.
    pub fn all_glyphs(&self) -> [(&'static str, &'static str); 34] {
        [
            ("inbox", self.inbox),
            ("outbox", self.outbox),
            ("threads", self.threads),
            ("channels", self.channels),
            ("groups", self.groups),
            ("public", self.public),
            ("encrypted", self.encrypted),
            ("creator", self.creator),
            ("draft", self.draft),
            ("check", self.check),
            ("error", self.error),
            ("lock_clock", self.lock_clock),
            ("history", self.history),
            ("sync", self.sync),
            ("block", self.block),
            ("account", self.account),
            ("wallet", self.wallet),
            ("key", self.key),
            ("help", self.help),
            ("exit", self.exit),
            ("magnify", self.magnify),
            ("menu", self.menu),
            ("keyboard", self.keyboard),
            ("cog", self.cog),
            ("refresh", self.refresh),
            ("swap", self.swap),
            ("copy", self.copy),
            ("lock_open", self.lock_open),
            ("chevron_left", self.chevron_left),
            ("chevron_right", self.chevron_right),
            ("arrow_up", self.arrow_up),
            ("arrow_down", self.arrow_down),
            ("arrow_left", self.arrow_left),
            ("arrow_right", self.arrow_right),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn every_glyph_is_single_cell() {
        for set in IconSet::ALL {
            for (name, glyph) in set.all_glyphs() {
                let w = UnicodeWidthStr::width(glyph);
                assert_eq!(
                    w, 1,
                    "glyph {name} = {glyph:?} has display width {w}, expected 1"
                );
            }
        }
    }

    #[test]
    fn every_glyph_is_single_scalar() {
        // One scalar means no VS16/ZWJ that could flip width to 2.
        for set in IconSet::ALL {
            for (name, glyph) in set.all_glyphs() {
                assert_eq!(
                    glyph.chars().count(),
                    1,
                    "glyph {name} = {glyph:?} must be a single scalar"
                );
            }
        }
    }

    #[test]
    fn nerd_is_byte_identical() {
        // The nerd set must match the historical codepoints exactly, so opting
        // into `nerd` looks the same as old taolk.
        assert_eq!(NERD.inbox, "\u{F02FB}");
        assert_eq!(NERD.channels, "\u{F0423}");
        assert_eq!(NERD.key, "\u{F030B}");
        assert_eq!(NERD.wallet, "\u{F0BDD}");
        assert_eq!(NERD.arrow_up, "\u{F005D}");
        assert_eq!(NERD.arrow_right, "\u{F0054}");
    }

    #[test]
    fn iconstyle_parse_is_case_insensitive() {
        assert_eq!(IconStyle::parse("nerd"), Some(IconStyle::Nerd));
        assert_eq!(IconStyle::parse("UNICODE"), Some(IconStyle::Unicode));
        assert_eq!(IconStyle::parse(" Ascii "), Some(IconStyle::Ascii));
        assert_eq!(IconStyle::parse("bogus"), None);
    }

    // `inbox` differs across all three themes, so it identifies which set we got.
    // (Pointer identity is unreliable here: `&NERD` on a `const` can be a fresh
    // promoted address each use.)
    #[test]
    fn iconstyle_set_maps_to_the_right_theme() {
        assert_eq!(IconStyle::Nerd.set().inbox, NERD.inbox);
        assert_eq!(IconStyle::Unicode.set().inbox, UNICODE.inbox);
        assert_eq!(IconStyle::Ascii.set().inbox, ASCII.inbox);
    }

    #[test]
    fn resolve_with_follows_precedence() {
        // TAOLK_ICONS wins over everything.
        assert_eq!(
            resolve_with("nerd", Some("ascii"), Some("1"), true).inbox,
            ASCII.inbox
        );
        // NERD_FONT=1 beats config.
        assert_eq!(
            resolve_with("unicode", None, Some("1"), true).inbox,
            NERD.inbox
        );
        // config value when no env.
        assert_eq!(resolve_with("nerd", None, None, true).inbox, NERD.inbox);
        // bad config falls through to the default.
        assert_eq!(resolve_with("bogus", None, None, true).inbox, UNICODE.inbox);
        // non-TTY downgrades the default to ascii.
        assert_eq!(resolve_with("", None, None, false).inbox, ASCII.inbox);
        // but an explicit choice beats the non-TTY rule.
        assert_eq!(resolve_with("nerd", None, None, false).inbox, NERD.inbox);
    }

    #[test]
    fn icon_pick_maps_to_the_set_field() {
        assert_eq!(Icon::Threads.pick(&ASCII), ASCII.threads);
        assert_eq!(Icon::Channels.pick(&UNICODE), UNICODE.channels);
        assert_eq!(Icon::Check.pick(&NERD), NERD.check);
        assert_eq!(Icon::Exit.pick(&ASCII), ASCII.exit);
    }

    // All Nerd Font PUA glyphs (U+F0000..U+FFFFD, written as \u{F0..}/\u{F1..} or
    // raw) must live only in this module. This keeps the codebase font-agnostic:
    // a contributor can't scatter a hard nerd glyph into a widget.
    #[test]
    fn no_nerd_pua_outside_registry() {
        use std::path::Path;

        fn scan(dir: &Path, offenders: &mut Vec<String>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    scan(&path, offenders);
                } else if path.extension().is_some_and(|e| e == "rs")
                    && !path.ends_with("ui/icons.rs")
                {
                    let text = std::fs::read_to_string(&path).unwrap();
                    for (i, line) in text.lines().enumerate() {
                        let escaped = ["\\u{F0", "\\u{f0", "\\u{F1", "\\u{f1"]
                            .iter()
                            .any(|p| line.contains(p));
                        let raw = line
                            .chars()
                            .any(|c| ('\u{F0000}'..='\u{FFFFD}').contains(&c));
                        if escaped || raw {
                            offenders.push(format!("{}:{}", path.display(), i + 1));
                        }
                    }
                }
            }
        }

        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders = Vec::new();
        scan(&src, &mut offenders);
        assert!(
            offenders.is_empty(),
            "Nerd PUA glyphs must live only in ui/icons.rs; found: {offenders:?}"
        );
    }
}
