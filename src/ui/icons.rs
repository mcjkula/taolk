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

// Legacy flat constants — still referenced by call sites until the migration
// commit replaces them with `icons().field`. Kept here so all PUA literals live
// in one module.
pub const INBOX: &str = "\u{F02FB}";
pub const OUTBOX: &str = "\u{F048A}";
pub const THREADS: &str = "\u{F0369}";
pub const CHANNELS: &str = "\u{F0423}";
pub const GROUPS: &str = "\u{F0849}";
pub const PUBLIC: &str = "\u{F0FC6}";
pub const ENCRYPTED: &str = "\u{F033E}";
pub const CREATOR: &str = "\u{F01A5}";
pub const DRAFT: &str = "\u{F03EB}";
pub const CHECK: &str = "\u{F012C}";
pub const ERROR: &str = "\u{F0028}";
pub const LOCK_CLOCK: &str = "\u{F097F}";
pub const HISTORY: &str = "\u{F02DA}";
pub const SYNC: &str = "\u{F04E6}";
pub const BLOCK: &str = "\u{F01A7}";
pub const ACCOUNT: &str = "\u{F0B55}";
pub const WALLET: &str = "\u{F0BDD}";
pub const KEY: &str = "\u{F030B}";
pub const HELP: &str = "\u{F0625}";
pub const EXIT: &str = "\u{F0206}";
pub const MAGNIFY: &str = "\u{F0349}";
pub const MENU: &str = "\u{F035C}";
pub const KEYBOARD: &str = "\u{F097B}";
pub const COG: &str = "\u{F0493}";
pub const REFRESH: &str = "\u{F0450}";
pub const SWAP: &str = "\u{F04E1}";
pub const COPY: &str = "\u{F018F}";
pub const LOCK_OPEN: &str = "\u{F0340}";
pub const CHEVRON_LEFT: &str = "\u{F0141}";
pub const CHEVRON_RIGHT: &str = "\u{F0142}";

/// Nerd Font Material Design glyphs (v3 nf-md-*). Needs a patched font.
// `allow` is dropped in the next commit once `icons()`/`resolve` consume the sets.
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
}
