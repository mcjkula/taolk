// Binary-only modules
mod app;
mod cli_fmt;
mod ui;

// Shared modules from the library
use taolk::{
    audio, chain, config, conversation, db, error, event, extrinsic, mirror, session, types, util,
    wallet,
};

use app::{App, Mode};
use chrono::{DateTime, Utc};
use clap::{ArgGroup, Parser, Subcommand};
use crossterm::ExecutableCommand;
use crossterm::event::{
    self as term_event, Event as TermEvent, KeyCode, KeyEvent, KeyModifiers, MouseEvent,
};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use schnorrkel::keys::{ExpansionMode, MiniSecretKey};
use std::io::stdout;
use std::sync::mpsc;
use std::time::Duration;

// ---------------------------------------------------------------------------
// TUI event types (will move to tui/event.rs in Phase 4)
// ---------------------------------------------------------------------------

enum TuiEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Tick,
    Core(event::Event),
}

struct TuiEventHandler {
    rx: mpsc::Receiver<TuiEvent>,
    core_tx: mpsc::Sender<event::Event>,
}

impl TuiEventHandler {
    fn new(tick_rate: Duration) -> Self {
        let (tui_tx, rx) = mpsc::channel();
        let (core_tx, core_rx) = mpsc::channel::<event::Event>();

        // Terminal polling thread
        let poll_tx = tui_tx.clone();
        std::thread::spawn(move || {
            loop {
                if term_event::poll(tick_rate).unwrap_or(false) {
                    match term_event::read() {
                        Ok(TermEvent::Key(key)) => {
                            if poll_tx.send(TuiEvent::Key(key)).is_err() {
                                return;
                            }
                        }
                        Ok(TermEvent::Mouse(mouse)) => {
                            if poll_tx.send(TuiEvent::Mouse(mouse)).is_err() {
                                return;
                            }
                        }
                        _ => {}
                    }
                }
                if poll_tx.send(TuiEvent::Tick).is_err() {
                    return;
                }
            }
        });

        // Core event forwarding thread
        std::thread::spawn(move || {
            while let Ok(event) = core_rx.recv() {
                if tui_tx.send(TuiEvent::Core(event)).is_err() {
                    return;
                }
            }
        });

        Self { rx, core_tx }
    }

    fn next(&self) -> Result<TuiEvent, mpsc::RecvError> {
        self.rx.recv()
    }

    fn core_sender(&self) -> mpsc::Sender<event::Event> {
        self.core_tx.clone()
    }
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(
    name = "taolk",
    about = "\u{03C4}alk \u{2014} encrypted messaging for Bittensor"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(long)]
    wallet: Option<String>,

    #[arg(long, default_value = "wss://entrypoint-finney.opentensor.ai:443")]
    node: String,

    #[arg(long, help = "SAMP mirror URL (optional, repeatable)")]
    mirror: Vec<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage wallets
    Wallet {
        #[command(subcommand)]
        action: WalletAction,
    },
    /// View and modify configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show all configuration values
    List,
    /// Get configuration values (all if no key given)
    Get {
        /// Key in dot-notation (e.g., network.node)
        key: Option<String>,
    },
    /// Set a configuration value
    Set {
        /// Key in dot-notation (e.g., network.node)
        key: String,
        /// Value(s) -- multiple values for list fields like network.mirrors
        #[arg(num_args = 1..)]
        value: Vec<String>,
    },
    /// Remove a key (revert to default)
    Unset {
        /// Key in dot-notation
        key: String,
    },
    /// Open config file in $EDITOR
    Edit,
    /// Show config file path
    Path,
}

#[derive(Subcommand)]
enum WalletAction {
    /// Create a new wallet with a fresh recovery phrase
    Create {
        /// Wallet name
        #[arg(long)]
        name: String,
        /// Password (skips interactive prompt)
        #[arg(long)]
        password: Option<String>,
    },
    /// Import a wallet from an existing recovery phrase or seed
    #[command(group = ArgGroup::new("source").required(true))]
    Import {
        /// Wallet name
        #[arg(long)]
        name: String,
        /// BIP39 recovery phrase (12 or 24 words)
        #[arg(long, group = "source")]
        mnemonic: Option<String>,
        /// Raw seed (64 hex characters)
        #[arg(long, group = "source")]
        seed: Option<String>,
        /// Password (skips interactive prompt)
        #[arg(long)]
        password: Option<String>,
    },
    /// List available wallets
    List,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let cfg = config::load();

    match cli.command {
        Some(Commands::Wallet { action }) => run_wallet_command(action),
        Some(Commands::Config { action }) => run_config_command(action),
        None => {
            let node = if cli.node != "wss://entrypoint-finney.opentensor.ai:443" {
                cli.node
            } else {
                cfg.network.node.clone()
            };
            let mirrors = if !cli.mirror.is_empty() {
                cli.mirror
            } else {
                cfg.network.mirrors.clone()
            };

            // Resolve wallet: CLI flag > config > auto-discover
            let wallet = cli.wallet.or_else(|| cfg.wallet.default.clone());
            if let Some(ref name) = wallet
                && !wallet::wallet_exists(name)
            {
                cli_fmt::error(&format!("Wallet '{name}' not found"));
                cli_fmt::hint("  Run `taolk wallet list` to see available wallets");
                std::process::exit(1);
            }
            let wallets = wallet::list_wallets();
            if wallet.is_none() && wallets.is_empty() {
                cli_fmt::error("No wallets found");
                cli_fmt::hint("  Run `taolk wallet create <name>` to create one");
                cli_fmt::hint("  Or  `taolk wallet import <name> --mnemonic \"...\"` to import");
                std::process::exit(1);
            }

            run_tui(wallet.as_deref(), &wallets, &node, &mirrors, &cfg)
        }
    }
}

// ---------------------------------------------------------------------------
// Wallet management (plain terminal)
// ---------------------------------------------------------------------------

fn run_wallet_command(action: WalletAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        WalletAction::Create { name, password } => cmd_wallet_create(&name, password),
        WalletAction::Import {
            name,
            mnemonic,
            seed,
            password,
        } => cmd_wallet_import(&name, mnemonic, seed, password),
        WalletAction::List => cmd_wallet_list(),
    }
}

fn cmd_wallet_create(
    wallet_name: &str,
    cli_password: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    use cli_fmt::*;

    if wallet::wallet_exists(wallet_name) {
        error(&format!("Wallet '{}' already exists", wallet_name));
        hint("  Use --wallet <other-name> to create a different wallet");
        std::process::exit(1);
    }

    header(&format!(
        "\u{03C4}alk \u{2014} Create wallet '{wallet_name}'"
    ));
    blank();

    let password = match cli_password {
        Some(p) => zeroize::Zeroizing::new(p),
        None => prompt_new_password()?,
    };

    let mnemonic = wallet::generate_mnemonic();
    let mut seed = wallet::seed_from_mnemonic(&mnemonic);
    let words: Vec<&str> = mnemonic.words().collect();

    wallet::create(wallet_name, &password, &seed)?;

    let msk = MiniSecretKey::from_bytes(&seed).unwrap();
    let kp = msk.expand_to_keypair(ExpansionMode::Ed25519);
    let address = util::ss58_from_pubkey(&types::Pubkey(kp.public.to_bytes()));
    zeroize::Zeroize::zeroize(&mut seed);

    blank();
    success("Wallet created");
    blank();
    label("Wallet", &format!("{BOLD}{wallet_name}{RESET}"));
    label_magenta("Address", &address);
    blank();
    header("Recovery phrase");
    hint("  Write this down. It is the ONLY way to recover your wallet.");
    blank();
    for (i, word) in words.iter().enumerate() {
        eprint!(
            "  {DIM}{:>2}.{RESET} {CYAN}{BOLD}{:<14}{RESET}",
            i + 1,
            word
        );
        if (i + 1) % 3 == 0 {
            eprintln!();
        }
    }
    if !words.len().is_multiple_of(3) {
        eprintln!();
    }
    blank();

    Ok(())
}

fn cmd_wallet_import(
    wallet_name: &str,
    mnemonic: Option<String>,
    seed_hex: Option<String>,
    cli_password: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    use cli_fmt::*;

    if wallet::wallet_exists(wallet_name) {
        error(&format!("Wallet '{}' already exists", wallet_name));
        hint("  Use --wallet <other-name> to import under a different name");
        std::process::exit(1);
    }

    header(&format!(
        "\u{03C4}alk \u{2014} Import wallet '{wallet_name}'"
    ));
    blank();

    let mut seed = if let Some(phrase) = mnemonic {
        let m = wallet::parse_mnemonic(&phrase)?;
        wallet::seed_from_mnemonic(&m)
    } else if let Some(hex) = seed_hex {
        wallet::seed_from_hex(&hex)?
    } else {
        error("Provide --mnemonic or --seed");
        std::process::exit(1);
    };

    let password = match cli_password {
        Some(p) => zeroize::Zeroizing::new(p),
        None => prompt_new_password()?,
    };
    wallet::create(wallet_name, &password, &seed)?;

    let msk = MiniSecretKey::from_bytes(&seed).unwrap();
    let kp = msk.expand_to_keypair(ExpansionMode::Ed25519);
    let address = util::ss58_from_pubkey(&types::Pubkey(kp.public.to_bytes()));
    zeroize::Zeroize::zeroize(&mut seed);

    blank();
    success("Wallet imported");
    blank();
    label("Wallet", &format!("{BOLD}{wallet_name}{RESET}"));
    label_magenta("Address", &address);
    blank();

    Ok(())
}

fn cmd_wallet_list() -> Result<(), Box<dyn std::error::Error>> {
    use cli_fmt::*;

    let wallets = wallet::list_wallets();
    if wallets.is_empty() {
        hint("No wallets found");
        hint("  Run `taolk wallet create` to create one");
    } else {
        header("\u{03C4}alk wallets");
        blank();
        for name in &wallets {
            let path = wallet::wallet_path(name);
            eprintln!(
                "  {CYAN}{BOLD}{name}{RESET}  {DIM}{}{RESET}",
                path.display()
            );
        }
        blank();
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Password prompts (plain terminal, rpassword)
// ---------------------------------------------------------------------------

fn prompt_new_password() -> Result<zeroize::Zeroizing<String>, Box<dyn std::error::Error>> {
    use cli_fmt::*;
    use zeroize::Zeroize;
    let password = rpassword::prompt_password(format!("  {YELLOW}Password:{RESET} "))?;
    if password.is_empty() {
        error("Password cannot be empty");
        std::process::exit(1);
    }
    let mut confirm = rpassword::prompt_password(format!("  {YELLOW}Confirm:{RESET}  "))?;
    if password != confirm {
        confirm.zeroize();
        error("Passwords do not match");
        std::process::exit(1);
    }
    confirm.zeroize();
    Ok(zeroize::Zeroizing::new(password))
}

// ---------------------------------------------------------------------------
// Configuration management (plain terminal)
// ---------------------------------------------------------------------------

fn run_config_command(action: ConfigAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        ConfigAction::List => cmd_config_list(),
        ConfigAction::Get { key: Some(key) } => cmd_config_get(&key),
        ConfigAction::Get { key: None } => cmd_config_list(),
        ConfigAction::Set { key, value } => cmd_config_set(&key, &value),
        ConfigAction::Unset { key } => cmd_config_unset(&key),
        ConfigAction::Edit => cmd_config_edit(),
        ConfigAction::Path => cmd_config_path(),
    }
}

fn cmd_config_list() -> Result<(), Box<dyn std::error::Error>> {
    use cli_fmt::*;

    let cfg = config::load();

    header("\u{03C4}alk configuration");
    blank();

    let mut current_section = "";
    for def in config::KEYS {
        if def.section != current_section {
            if !current_section.is_empty() {
                blank();
            }
            current_section = def.section;
            eprintln!("  {BOLD}{WHITE}[{current_section}]{RESET}");
        }

        let value = config::get_value(&cfg, def.key);
        let user_set = config::is_user_set(def.key);
        let padded_field = format!("{:<19}", def.field);

        if user_set {
            label_cyan(&padded_field, &value);
        } else {
            label_dim(&padded_field, &format!("{value}  (default)"));
        }
    }

    blank();
    hint(&format!("  {}", config::config_path().display()));
    blank();
    Ok(())
}

fn cmd_config_get(key: &str) -> Result<(), Box<dyn std::error::Error>> {
    if config::lookup_key(key).is_none() {
        unknown_key_error(key);
    }
    let cfg = config::load();
    println!("{}", config::get_value(&cfg, key));
    Ok(())
}

fn cmd_config_set(key: &str, values: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use cli_fmt::*;

    if config::lookup_key(key).is_none() {
        unknown_key_error(key);
    }

    match config::set_key(key, values) {
        Ok(display_val) => {
            success(&format!("{key} = {display_val}"));
            hint(&format!("  {}", config::config_path().display()));
        }
        Err(msg) => {
            error(&msg);
            std::process::exit(1);
        }
    }
    Ok(())
}

fn cmd_config_unset(key: &str) -> Result<(), Box<dyn std::error::Error>> {
    use cli_fmt::*;

    if config::lookup_key(key).is_none() {
        unknown_key_error(key);
    }

    match config::unset_key(key) {
        Ok(default_val) => {
            success(&format!("{key} reset to default ({default_val})"));
            hint(&format!("  {}", config::config_path().display()));
        }
        Err(msg) => {
            error(&msg);
            std::process::exit(1);
        }
    }
    Ok(())
}

fn cmd_config_edit() -> Result<(), Box<dyn std::error::Error>> {
    use cli_fmt::*;

    let path = config::config_path();
    if !path.exists() {
        config::save(&config::Config::default())
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
    }

    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| {
            error("$EDITOR is not set");
            hint("  Set EDITOR or VISUAL environment variable");
            std::process::exit(1);
        });

    let status = std::process::Command::new(&editor).arg(&path).status()?;
    if !status.success() {
        error(&format!("{editor} exited with {status}"));
        std::process::exit(1);
    }
    Ok(())
}

fn cmd_config_path() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", config::config_path().display());
    Ok(())
}

fn unknown_key_error(key: &str) -> ! {
    use cli_fmt::*;
    error(&format!("Unknown key: {key}"));
    if let Some(def) = config::suggest_key(key) {
        hint(&format!(
            "  Did you mean '{}'? ({})",
            def.key, def.description
        ));
    }
    hint("  Run 'taolk config list' to see all keys");
    std::process::exit(1);
}

// ---------------------------------------------------------------------------
// TUI client
// ---------------------------------------------------------------------------

fn run_tui(
    preselected: Option<&str>,
    wallets: &[String],
    node_url: &str,
    mirror_urls: &[String],
    cfg: &config::Config,
) -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    if cfg.ui.mouse {
        stdout().execute(EnableMouseCapture)?;
    }
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let events = TuiEventHandler::new(Duration::from_millis(250));

    let mut first_login = true;
    let mut current_wallet = preselected.unwrap_or("").to_string();

    loop {
        let result = if first_login {
            run_lock_screen(&mut terminal, &events, wallets, preselected)?
        } else {
            run_lock_screen(&mut terminal, &events, &[], Some(&current_wallet))?
        };

        let (wallet_name, seed) = match result {
            Some(r) => r,
            None => break,
        };

        first_login = false;
        current_wallet = wallet_name.clone();

        let quit = run_session(
            &mut terminal,
            &events,
            &seed,
            &wallet_name,
            node_url,
            mirror_urls,
            cfg,
        )?;
        drop(seed);
        if quit {
            break;
        }
    }

    if cfg.ui.mouse {
        stdout().execute(DisableMouseCapture)?;
    }
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

type UnlockResult = Option<(String, zeroize::Zeroizing<[u8; 32]>)>;

fn run_lock_screen(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    events: &TuiEventHandler,
    wallets: &[String],
    preselected: Option<&str>,
) -> Result<UnlockResult, Box<dyn std::error::Error>> {
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::Paragraph;

    const LOGO: &[&str] = &[
        "   \u{2591}\u{2588}\u{2588}                                     \u{2591}\u{2588}\u{2588} \u{2591}\u{2588}\u{2588}",
        "   \u{2591}\u{2588}\u{2588}                                     \u{2591}\u{2588}\u{2588} \u{2591}\u{2588}\u{2588}",
        "\u{2591}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}  \u{2591}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}    \u{2591}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}   \u{2591}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}   \u{2591}\u{2588}\u{2588} \u{2591}\u{2588}\u{2588}    \u{2591}\u{2588}\u{2588}",
        "   \u{2591}\u{2588}\u{2588}          \u{2591}\u{2588}\u{2588}  \u{2591}\u{2588}\u{2588}    \u{2591}\u{2588}\u{2588}       \u{2591}\u{2588}\u{2588}  \u{2591}\u{2588}\u{2588} \u{2591}\u{2588}\u{2588}   \u{2591}\u{2588}\u{2588}",
        "   \u{2591}\u{2588}\u{2588}     \u{2591}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}  \u{2591}\u{2588}\u{2588}    \u{2591}\u{2588}\u{2588}  \u{2591}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}  \u{2591}\u{2588}\u{2588} \u{2591}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}",
        "   \u{2591}\u{2588}\u{2588}    \u{2591}\u{2588}\u{2588}   \u{2591}\u{2588}\u{2588}  \u{2591}\u{2588}\u{2588}    \u{2591}\u{2588}\u{2588} \u{2591}\u{2588}\u{2588}   \u{2591}\u{2588}\u{2588}  \u{2591}\u{2588}\u{2588} \u{2591}\u{2588}\u{2588}   \u{2591}\u{2588}\u{2588}",
        "    \u{2591}\u{2588}\u{2588}\u{2588}\u{2588}  \u{2591}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2591}\u{2588}\u{2588}  \u{2591}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}   \u{2591}\u{2588}\u{2588}\u{2588}\u{2588}\u{2588}\u{2591}\u{2588}\u{2588} \u{2591}\u{2588}\u{2588} \u{2591}\u{2588}\u{2588}    \u{2591}\u{2588}\u{2588}",
    ];
    const SUBTITLE: &str = "\u{03C4}alk \u{2014} encrypted messaging for Bit\u{03C4}ensor";

    let show_carousel = preselected.is_none() && wallets.len() > 1;
    let mut wallet_idx: usize = 0;
    let mut inserting = false;
    let mut password = zeroize::Zeroizing::new(String::new());
    let mut error_msg: Option<String> = None;

    // For single wallet or preselected: determine the wallet name
    let fixed_wallet = preselected.map(String::from).or_else(|| {
        if wallets.len() == 1 {
            Some(wallets[0].clone())
        } else {
            None
        }
    });

    loop {
        let current_wallet = fixed_wallet
            .clone()
            .unwrap_or_else(|| wallets.get(wallet_idx).cloned().unwrap_or_default());

        terminal.draw(|frame| {
            let area = frame.area();
            let w = area.width as usize;
            let h = area.height as usize;

            let content_height = 18;
            let top_pad = h.saturating_sub(content_height) / 2;
            let logo_display_width = 55;

            let mut lines: Vec<Line> = Vec::new();
            for _ in 0..top_pad {
                lines.push(Line::raw(""));
            }

            // Logo
            let logo_pad = " ".repeat(w.saturating_sub(logo_display_width) / 2);
            for logo_line in LOGO {
                lines.push(Line::styled(
                    format!("{logo_pad}{logo_line}"),
                    Style::default().fg(Color::Cyan),
                ));
            }

            lines.push(Line::raw(""));

            // Subtitle
            let sub_chars = SUBTITLE.chars().count();
            let sub_pad = " ".repeat(w.saturating_sub(sub_chars) / 2);
            lines.push(Line::styled(
                format!("{sub_pad}{SUBTITLE}"),
                Style::default().fg(Color::DarkGray),
            ));

            lines.push(Line::raw(""));
            lines.push(Line::raw(""));

            // Wallet display: carousel or static name
            if show_carousel && !inserting {
                // Horizontal carousel: 3-slot window
                let win_start = wallet_idx
                    .saturating_sub(1)
                    .min(wallets.len().saturating_sub(3));
                let win_end = (win_start + 3).min(wallets.len());

                let mut spans: Vec<Span> = Vec::new();
                if win_start > 0 {
                    spans.push(Span::styled(
                        "\u{2039}  ",
                        Style::default().fg(Color::DarkGray),
                    ));
                } else {
                    spans.push(Span::raw("   "));
                }
                for (i, name) in wallets[win_start..win_end].iter().enumerate() {
                    if i > 0 {
                        spans.push(Span::styled(
                            "  \u{2014}  ",
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                    if win_start + i == wallet_idx {
                        spans.push(Span::styled(
                            name.clone(),
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        ));
                    } else {
                        spans.push(Span::styled(
                            name.clone(),
                            Style::default().fg(Color::DarkGray),
                        ));
                    }
                }
                if win_end < wallets.len() {
                    spans.push(Span::styled(
                        "  \u{203A}",
                        Style::default().fg(Color::DarkGray),
                    ));
                } else {
                    spans.push(Span::raw("  "));
                }

                // Center the carousel line
                let carousel_width: usize = spans.iter().map(|s| s.content.chars().count()).sum();
                let carousel_pad = " ".repeat(w.saturating_sub(carousel_width) / 2);
                let mut centered = vec![Span::raw(carousel_pad)];
                centered.extend(spans);
                lines.push(Line::from(centered));
            } else {
                // Static wallet name
                let wp = " ".repeat(w.saturating_sub(current_wallet.len()) / 2);
                lines.push(Line::styled(
                    format!("{wp}{current_wallet}"),
                    Style::default().fg(Color::White),
                ));
            }

            lines.push(Line::raw(""));

            // Password prompt
            let prompt = "Password: ";
            let prompt_color = if inserting {
                Color::White
            } else {
                Color::DarkGray
            };
            let pp = w.saturating_sub(prompt.len()) / 2;
            let pp_str = " ".repeat(pp);
            lines.push(Line::from(vec![
                Span::raw(pp_str),
                Span::styled(prompt, Style::default().fg(prompt_color)),
            ]));

            // Error
            if let Some(err) = &error_msg {
                lines.push(Line::raw(""));
                let ep = " ".repeat(w.saturating_sub(err.len()) / 2);
                lines.push(Line::styled(
                    format!("{ep}{err}"),
                    Style::default().fg(Color::Red),
                ));
            } else {
                lines.push(Line::raw(""));
                lines.push(Line::raw(""));
            }

            // Hints
            let hints = if inserting {
                "Enter unlock \u{00B7} Esc back"
            } else if show_carousel {
                "\u{2190}/\u{2192} select \u{00B7} i unlock \u{00B7} q quit"
            } else {
                "i unlock \u{00B7} q quit"
            };
            let hp = " ".repeat(w.saturating_sub(hints.chars().count()) / 2);
            lines.push(Line::styled(
                format!("{hp}{hints}"),
                Style::default().fg(Color::DarkGray),
            ));

            frame.render_widget(Paragraph::new(lines), area);

            if inserting {
                let cursor_y = area.y + top_pad as u16 + 7 + 1 + 1 + 2 + 1 + 1;
                let cursor_x = area.x + pp as u16 + prompt.len() as u16;
                if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
                    frame.set_cursor_position((cursor_x, cursor_y));
                }
            }
        })?;

        match events.next()? {
            TuiEvent::Key(key) if inserting => match key.code {
                KeyCode::Enter => match wallet::open(&current_wallet, &password) {
                    Ok(new_seed) => {
                        password.clear();
                        return Ok(Some((current_wallet, new_seed)));
                    }
                    Err(wallet::WalletError::WrongPassword) => {
                        password.clear();
                        error_msg = Some("Wrong password".into());
                    }
                    Err(e) => {
                        password.clear();
                        error_msg = Some(format!("{e}"));
                    }
                },
                KeyCode::Esc => {
                    inserting = false;
                    password.clear();
                    error_msg = None;
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    password.clear();
                    return Ok(None);
                }
                KeyCode::Char(c) => {
                    error_msg = None;
                    password.push(c);
                }
                KeyCode::Backspace => {
                    error_msg = None;
                    password.pop();
                }
                _ => {}
            },
            TuiEvent::Key(key) => match key.code {
                KeyCode::Char('i') | KeyCode::Enter => {
                    inserting = true;
                    error_msg = None;
                }
                KeyCode::Left | KeyCode::Char('h') if show_carousel => {
                    wallet_idx = wallet_idx.saturating_sub(1);
                    error_msg = None;
                }
                KeyCode::Right | KeyCode::Char('l') if show_carousel => {
                    if wallet_idx + 1 < wallets.len() {
                        wallet_idx += 1;
                    }
                    error_msg = None;
                }
                KeyCode::Char('q') => {
                    return Ok(None);
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(None);
                }
                _ => {}
            },
            TuiEvent::Tick | TuiEvent::Core(_) | TuiEvent::Mouse(_) => {}
        }
    }
}

/// Session: takes seed, builds app, runs event loop. Returns true=quit, false=lock.
fn run_session(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    events: &TuiEventHandler,
    seed: &[u8; 32],
    wallet_name: &str,
    node_url: &str,
    mirror_urls: &[String],
    cfg: &config::Config,
) -> Result<bool, Box<dyn std::error::Error>> {
    let msk = MiniSecretKey::from_bytes(seed).map_err(|e| format!("Invalid seed: {e}"))?;
    let keypair = msk.expand_to_keypair(ExpansionMode::Ed25519);
    let my_pubkey = types::Pubkey(keypair.public.to_bytes());

    let rt = tokio::runtime::Runtime::new()?;

    let chain_info = rt
        .block_on(extrinsic::fetch_chain_info(node_url))
        .map_err(|e| format!("Failed to fetch chain info: {e}"))?;

    let (token_symbol, token_decimals) = match rt.block_on(extrinsic::fetch_token_info(node_url)) {
        Ok(info) => info,
        Err(e) => {
            eprintln!("Warning: Could not fetch token info: {e}. Defaulting to TAO/9.");
            ("TAO".into(), 9)
        }
    };

    let db = db::Db::open(wallet_name, seed, &chain_info.genesis_hash)?;
    let session = session::Session::new(
        keypair,
        zeroize::Zeroizing::new(*seed),
        node_url.to_string(),
        chain_info.clone(),
        db,
    );
    let audio = audio::Audio::from_config(&cfg.notifications);
    let mut app = App::new(session, audio);
    app.session.token_symbol = token_symbol;
    app.session.token_decimals = token_decimals;
    app.sidebar_width = cfg.ui.sidebar_width;
    app.timestamp_format = cfg.ui.timestamp_format.clone();
    app.date_format = cfg.ui.date_format.clone();
    app.session.load_from_db();

    if let Ok(bal) = rt.block_on(extrinsic::fetch_balance(
        node_url,
        &my_pubkey,
        &chain_info.account_info_layout,
    )) {
        app.session.balance = Some(bal);
    }

    let event_tx = events.core_sender();
    let lock_timeout = std::time::Duration::from_secs(cfg.security.lock_timeout);
    let mut last_activity = std::time::Instant::now();

    // Spawn chain subscription
    {
        let url = node_url.to_string();
        let tx = event_tx.clone();
        let sc = zeroize::Zeroizing::new(*seed);
        rt.spawn(async move {
            let _ = tx.send(event::Event::Status("Connected".into()));
            chain::subscribe_blocks(&url, my_pubkey, sc, tx).await;
        });
    }

    // Mirror sync
    app.session.has_mirror = !mirror_urls.is_empty();
    if app.session.has_mirror {
        let subscribed: Vec<types::BlockRef> =
            app.session.channels.iter().map(|c| c.channel_ref).collect();
        for mirror_url in mirror_urls {
            let url = mirror_url.clone();
            let sc = zeroize::Zeroizing::new(*seed);
            let pubkey = my_pubkey;
            let channels = subscribed.clone();
            let tx = event_tx.clone();
            rt.spawn(async move {
                mirror::sync(&url, 42, &sc, &pubkey, channels, 0, tx).await;
            });
        }
    } else {
        app.sound_armed = true;
    }

    app.set_status("Connected");

    // Event loop -- runs until quit or lock
    while app.running {
        terminal.draw(|frame| ui::render(frame, &app))?;

        // Check lock timeout (0 = disabled)
        if cfg.security.lock_timeout > 0 && last_activity.elapsed() > lock_timeout {
            return Ok(false); // lock
        }

        match events.next()? {
            TuiEvent::Key(key) => {
                last_activity = std::time::Instant::now();
                if (key.code == KeyCode::Char('l') && key.modifiers.contains(KeyModifiers::CONTROL))
                    || key.code == KeyCode::Char('\x0c')
                {
                    return Ok(false); // Ctrl+L: lock
                }
                handle_key(&mut app, key, &event_tx, &rt);
            }
            TuiEvent::Mouse(mouse) => {
                last_activity = std::time::Instant::now();
                handle_mouse(&mut app, mouse, terminal);
            }
            TuiEvent::Tick => {
                app.frame = app.frame.wrapping_add(1);
            }
            TuiEvent::Core(event::Event::BlockUpdate(num)) => {
                let new_block = num != app.session.block_number;
                if new_block {
                    app.block_changed_at = app.frame;
                }
                app.session.block_number = num;
                if new_block {
                    let url = node_url.to_string();
                    let pk = my_pubkey;
                    let layout = app.session.chain_info.account_info_layout.clone();
                    let tx = event_tx.clone();
                    rt.spawn(async move {
                        if let Ok(bal) = extrinsic::fetch_balance(&url, &pk, &layout).await {
                            let _ = tx.send(event::Event::BalanceUpdated(bal));
                        }
                    });
                }
            }
            TuiEvent::Core(event::Event::NewMessage {
                sender,
                content_type: ct,
                recipient,
                decrypted_body,
                thread_ref,
                reply_to,
                continues,
                block_number,
                ext_index,
                timestamp,
            }) => {
                let body = match decrypted_body {
                    Some(text) => text,
                    None => continue,
                };
                let ts = DateTime::<Utc>::from_timestamp(timestamp as i64, 0).unwrap_or_default();
                let sender_ss58 = util::ss58_short(&sender);
                let is_mine = sender == app.session.pubkey();
                let kind = ct & 0x0F;

                match kind {
                    0x00 | 0x01 => {
                        // Public or encrypted
                        app.session.add_inbox_message(
                            sender,
                            recipient,
                            ts,
                            body,
                            kind,
                            types::BlockRef {
                                block: block_number,
                                index: ext_index,
                            },
                        );
                    }
                    0x02 => {
                        // Thread
                        app.session.add_thread_message(
                            sender,
                            recipient,
                            thread_ref,
                            conversation::NewMessage {
                                sender_ss58,
                                timestamp: ts,
                                body,
                                reply_to,
                                continues,
                                block_number,
                                ext_index,
                            },
                        );
                    }
                    _ => {}
                }

                if app.sound_armed && !is_mine {
                    app.audio.play(audio::Sound::Dm);
                }
            }
            TuiEvent::Core(event::Event::NewChannelMessage {
                sender,
                sender_ss58,
                channel_ref,
                body,
                reply_to,
                continues,
                block_number,
                ext_index,
                timestamp,
            }) => {
                let ts = DateTime::<Utc>::from_timestamp(timestamp as i64, 0).unwrap_or_default();
                let is_mine = sender_ss58 == util::ss58_short(&app.session.pubkey());
                let mentioned = util::body_mentions(&body, app.session.ss58());
                app.session.peer_pubkeys.insert(sender_ss58.clone(), sender);
                app.session.db.upsert_peer(&sender_ss58, &sender);
                app.session.add_channel_message(
                    channel_ref,
                    conversation::NewMessage {
                        sender_ss58,
                        timestamp: ts,
                        body,
                        reply_to,
                        continues,
                        block_number,
                        ext_index,
                    },
                );
                if app.sound_armed && !is_mine {
                    let sound = if mentioned {
                        audio::Sound::Mention
                    } else {
                        audio::Sound::Ambient
                    };
                    app.audio.play(sound);
                }
            }
            TuiEvent::Core(event::Event::ChannelDiscovered {
                name,
                description,
                creator_ss58,
                channel_ref,
            }) => {
                app.session
                    .discover_channel(name, description, creator_ss58, channel_ref);
            }
            TuiEvent::Core(event::Event::GroupDiscovered {
                creator_pubkey,
                group_ref,
                members,
            }) => {
                app.session
                    .db
                    .insert_group(group_ref, &creator_pubkey, &members);
                app.session
                    .discover_group(creator_pubkey, group_ref, members);
            }
            TuiEvent::Core(event::Event::NewGroupMessage {
                sender,
                sender_ss58,
                group_ref,
                body,
                reply_to,
                continues,
                block_number,
                ext_index,
                timestamp,
            }) => {
                let ts = DateTime::<Utc>::from_timestamp(timestamp as i64, 0).unwrap_or_default();
                let is_mine = sender_ss58 == util::ss58_short(&app.session.pubkey());
                let mentioned = util::body_mentions(&body, app.session.ss58());
                app.session.peer_pubkeys.insert(sender_ss58.clone(), sender);
                app.session.db.upsert_peer(&sender_ss58, &sender);
                app.session.add_group_message(
                    group_ref,
                    conversation::NewMessage {
                        sender_ss58,
                        timestamp: ts,
                        body,
                        reply_to,
                        continues,
                        block_number,
                        ext_index,
                    },
                );
                if app.sound_armed && !is_mine {
                    let sound = if mentioned {
                        audio::Sound::Mention
                    } else {
                        audio::Sound::Ambient
                    };
                    app.audio.play(sound);
                }
            }
            TuiEvent::Core(event::Event::SubmitRemark { remark }) => {
                let url = app.session.node_url.clone();
                let kp = app.session.keypair.clone();
                let ss58 = app.session.my_ss58.clone();
                let ci = chain_info.clone();
                let tx = event_tx.clone();
                rt.spawn(async move {
                    match extrinsic::submit_remark(&url, &remark, &kp, &ss58, &ci).await {
                        Ok(_) => {
                            let _ = tx.send(event::Event::MessageSent);
                        }
                        Err(e) => {
                            let _ = tx.send(event::Event::Error(format!("Send failed: {e}")));
                        }
                    }
                });
            }
            TuiEvent::Core(event::Event::FetchChannelMirror { channel_ref }) => {
                if let Some(mirror_url) = mirror_urls.first() {
                    app.set_status("Loading...");
                    let url = mirror_url.clone();
                    let tx = event_tx.clone();
                    rt.spawn(async move {
                        mirror::fetch_channel(&url, channel_ref, tx).await;
                    });
                }
            }
            TuiEvent::Core(event::Event::FetchBlock { block_ref }) => {
                app.set_status("Loading...");
                let url = node_url.to_string();
                let tx = event_tx.clone();
                let sc = zeroize::Zeroizing::new(*seed);
                rt.spawn(async move {
                    chain::fetch_and_process_extrinsic(
                        &url,
                        block_ref.block,
                        block_ref.index,
                        my_pubkey,
                        sc,
                        tx.clone(),
                    )
                    .await;
                    let _ = tx.send(event::Event::GapsRefreshed);
                });
            }
            TuiEvent::Core(event::Event::GapsRefreshed) => {
                for i in 0..app.session.threads.len() {
                    app.session.refresh_gaps(i);
                }
                for i in 0..app.session.channels.len() {
                    app.session.refresh_channel_gaps(i);
                }
                for i in 0..app.session.groups.len() {
                    app.session.refresh_group_gaps(i);
                }
                app.set_status("Loaded");
            }
            TuiEvent::Core(event::Event::FeeEstimated {
                fee_display,
                fee_raw,
            }) => {
                if app.mode == Mode::Confirm {
                    app.pending_fee = Some(fee_display);
                }
                if let Some(raw) = fee_raw {
                    app.last_fee = Some(raw);
                }
            }
            TuiEvent::Core(event::Event::MessageSent) => {
                app.sending = false;
                app.pending_text = None;
                app.pending_view = None;
                let fee_info = app
                    .last_fee
                    .map(|f| {
                        format!(
                            " (-{})",
                            util::format_fee(
                                f,
                                app.session.token_decimals,
                                &app.session.token_symbol
                            )
                        )
                    })
                    .unwrap_or_default();
                app.set_status(format!("Confirmed{fee_info}"));
                app.last_fee = None;
                // Refresh balance after send
                let url = node_url.to_string();
                let pk = my_pubkey;
                let tx = event_tx.clone();
                let layout = app.session.chain_info.account_info_layout.clone();
                rt.spawn(async move {
                    if let Ok(bal) = extrinsic::fetch_balance(&url, &pk, &layout).await {
                        let _ = tx.send(event::Event::BalanceUpdated(bal));
                    }
                });
            }
            TuiEvent::Core(event::Event::BalanceUpdated(bal)) => {
                if app.session.balance != Some(bal) {
                    app.balance_decreased = app.session.balance.is_some_and(|prev| bal < prev);
                    app.balance_changed_at = app.frame;
                }
                app.session.balance = Some(bal);
            }
            TuiEvent::Core(event::Event::Status(msg)) => {
                app.set_status(msg);
            }
            TuiEvent::Core(event::Event::CatchupComplete) => {
                app.sound_armed = true;
            }
            TuiEvent::Core(event::Event::Error(e)) => {
                if app.sending {
                    if let Some(result) = app.session.cleanup_pending()
                        && (result
                            .removed_thread
                            .is_some_and(|idx| app.view == app::View::Thread(idx))
                            || result
                                .removed_channel
                                .is_some_and(|idx| app.view == app::View::Channel(idx))
                            || result
                                .removed_group
                                .is_some_and(|idx| app.view == app::View::Group(idx)))
                    {
                        app.view = app::View::Inbox;
                    }
                    if let Some(text) = app.pending_text.take() {
                        app.input = text;
                        app.cursor_pos = app.input.len();
                    }
                    app.sending = false;
                    app.pending_view = None;
                }
                app.set_error(e);
            }
        }
    }

    Ok(true) // quit
}

// ---------------------------------------------------------------------------
// SAMP remark builder
// ---------------------------------------------------------------------------

fn build_send_remark(app: &App, text: &str) -> error::Result<Vec<u8>> {
    // Standalone public/encrypted message
    if let (Some((pubkey, _)), Some(ct)) = (&app.msg_recipient, app.msg_type) {
        return match ct {
            0x01 => app.session.build_public_message(pubkey, text),
            0x02 => app.session.build_encrypted_message(pubkey, text),
            _ => Err(error::SdkError::Other("Invalid message type".into())),
        };
    }

    // New thread (msg_recipient set, no msg_type)
    if let (Some((pubkey, _)), None) = (&app.msg_recipient, app.msg_type) {
        return app.session.build_thread_root(pubkey, text);
    }

    match app.view {
        app::View::Thread(idx) => app.session.build_thread_reply(idx, text),
        app::View::Channel(idx) => app.session.build_channel_message(idx, text),
        app::View::Group(idx) => {
            let group = app
                .session
                .groups
                .get(idx)
                .ok_or_else(|| error::SdkError::NotFound("No group selected".into()))?;
            if group.group_ref.is_zero() {
                app.session.build_group_create(&group.members.clone(), text)
            } else {
                app.session.build_group_message(idx, text)
            }
        }
        _ => Err(error::SdkError::Other("Cannot send from this view".into())),
    }
}

/// Shared text editing for all input modes. Returns true if the key was handled.
fn handle_text_input(app: &mut App, key: crossterm::event::KeyEvent) -> bool {
    match key.code {
        KeyCode::Char(c) => {
            app.input.insert(app.cursor_pos, c);
            app.cursor_pos += 1;
        }
        KeyCode::Backspace => {
            if app.cursor_pos > 0 {
                app.cursor_pos -= 1;
                app.input.remove(app.cursor_pos);
            }
        }
        KeyCode::Delete => {
            if app.cursor_pos < app.input.len() {
                app.input.remove(app.cursor_pos);
            }
        }
        KeyCode::Left => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                // Ctrl+Left: jump to previous word boundary
                app.cursor_pos = app.input[..app.cursor_pos].rfind(' ').unwrap_or(0);
            } else {
                app.cursor_pos = app.cursor_pos.saturating_sub(1);
            }
        }
        KeyCode::Right => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                // Ctrl+Right: jump to next word boundary
                app.cursor_pos = app.input[app.cursor_pos..]
                    .find(' ')
                    .map(|p| app.cursor_pos + p + 1)
                    .unwrap_or(app.input.len());
            } else if app.cursor_pos < app.input.len() {
                app.cursor_pos += 1;
            }
        }
        KeyCode::Home => app.cursor_pos = 0,
        KeyCode::End => app.cursor_pos = app.input.len(),
        _ => return false,
    }
    app.contact_idx = 0;
    true
}

// ---------------------------------------------------------------------------
// Input handlers
// ---------------------------------------------------------------------------

fn handle_mouse(
    app: &mut App,
    mouse: crossterm::event::MouseEvent,
    terminal: &Terminal<CrosstermBackend<std::io::Stdout>>,
) {
    use crossterm::event::{MouseButton, MouseEventKind};

    let term_size = terminal.size().unwrap_or_default();
    let sidebar_width: u16 = if app.show_sidebar {
        app.sidebar_width
    } else {
        0
    };
    let input_area_y = term_size.height.saturating_sub(4);

    if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
        let x = mouse.column;
        let y = mouse.row;

        let hit = app
            .sender_click_regions
            .borrow()
            .iter()
            .find(|(row, c0, c1, _)| *row == y && x >= *c0 && x < *c1)
            .map(|(_, _, _, ss58)| ss58.clone());
        if let Some(short) = hit {
            let pk = app.session.peer_pubkeys.get(&short).copied();
            copy_sender(app, &short, pk.as_ref());
            return;
        }

        if app.show_sidebar && x < sidebar_width {
            let row = y.saturating_sub(1) as usize;
            app.select_sidebar_row(row);
        } else if y >= input_area_y && !app.sending {
            app.load_draft();
            app.mode = Mode::Insert;
        }
    }
}

fn handle_key(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    send_tx: &std::sync::mpsc::Sender<event::Event>,
    rt: &tokio::runtime::Runtime,
) {
    match app.mode {
        Mode::Normal => handle_normal_key(app, key, send_tx),
        Mode::Insert => handle_insert_key(app, key, send_tx, rt),
        Mode::Confirm => handle_confirm_key(app, key, send_tx),
        Mode::Compose => handle_compose_key(app, key),
        Mode::Message => handle_message_key(app, key),
        Mode::CreateChannel => handle_create_channel_key(app, key),
        Mode::CreateChannelDesc => handle_create_channel_desc_key(app, key, send_tx, rt),
        Mode::CreateGroupMembers => handle_create_group_members_key(app, key, send_tx),
        Mode::Search => handle_search_key(app, key),
        Mode::SenderPicker => handle_sender_picker_key(app, key),
    }
}

fn handle_normal_key(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    send_tx: &std::sync::mpsc::Sender<event::Event>,
) {
    // Channel directory: browse discovered channels or type a channel ID
    if app.view == app::View::ChannelDir {
        match key.code {
            KeyCode::Down if app.channel_dir_input.is_empty() => {
                if !app.session.known_channels.is_empty() {
                    app.channel_dir_cursor =
                        (app.channel_dir_cursor + 1).min(app.session.known_channels.len() - 1);
                }
            }
            KeyCode::Up if app.channel_dir_input.is_empty() => {
                app.channel_dir_cursor = app.channel_dir_cursor.saturating_sub(1);
            }
            KeyCode::Enter => {
                if !app.channel_dir_input.is_empty() {
                    // Manual ref input: subscribe
                    match parse_channel_ref(&app.channel_dir_input) {
                        Ok(channel_ref) => {
                            let idx = app.session.subscribe_channel(channel_ref);
                            app.view = app::View::Channel(idx);
                            app.set_status(format!(
                                "Subscribed to #{}",
                                app.session.channels[idx].name
                            ));
                            app.channel_dir_input.clear();
                            let _ = send_tx.send(event::Event::FetchBlock {
                                block_ref: channel_ref,
                            });
                            if app.session.has_mirror {
                                let _ =
                                    send_tx.send(event::Event::FetchChannelMirror { channel_ref });
                            }
                        }
                        Err(e) => {
                            app.set_error(format!("Invalid channel ref: {e}"));
                        }
                    }
                } else if let Some(info) = app.session.known_channels.get(app.channel_dir_cursor) {
                    // Toggle subscribe/unsubscribe
                    let channel_ref = info.channel_ref;
                    if app.session.is_subscribed(&channel_ref) {
                        // Unsubscribe
                        if let Some(idx) = app.session.channel_idx(&channel_ref)
                            && let Some(name) = app.session.unsubscribe_channel(idx)
                        {
                            app.set_status(format!("Left #{name}"));
                        }
                    } else {
                        // Subscribe
                        let idx = app.session.subscribe_channel(channel_ref);
                        app.set_status(format!(
                            "Subscribed to #{}",
                            app.session.channels[idx].name
                        ));
                        let _ = send_tx.send(event::Event::FetchBlock {
                            block_ref: channel_ref,
                        });
                        if app.session.has_mirror {
                            let _ = send_tx.send(event::Event::FetchChannelMirror { channel_ref });
                        }
                    }
                }
            }
            KeyCode::Char('c') if app.channel_dir_input.is_empty() => {
                // Create new channel (from inside directory)
                if app.sending {
                    app.set_error("Still sending previous message");
                    return;
                }
                app.input.clear();
                app.cursor_pos = 0;
                app.mode = Mode::CreateChannel;
            }
            KeyCode::Esc => {
                if app.channel_dir_input.is_empty() {
                    app.view = app::View::Inbox;
                } else {
                    app.channel_dir_input.clear();
                }
            }
            KeyCode::Backspace => {
                app.channel_dir_input.pop();
            }
            KeyCode::Char('q') if app.channel_dir_input.is_empty() => {
                app.running = false;
            }
            KeyCode::Char(c) if c.is_ascii_digit() || c == ':' => {
                app.channel_dir_input.push(c);
            }
            // j/k/Tab/BackTab fall through to sidebar navigation below
            KeyCode::Char('j') | KeyCode::Char('k') | KeyCode::Tab | KeyCode::BackTab => {}
            _ => {
                return;
            }
        }
    }

    if key.code != KeyCode::Char('q') {
        app.quit_confirm = false;
    }
    match key.code {
        KeyCode::Char('q') => {
            let has_drafts = app.session.threads.iter().any(|c| !c.draft.is_empty())
                || app.session.channels.iter().any(|c| !c.draft.is_empty());
            if has_drafts && !app.quit_confirm {
                app.set_status("Unsaved drafts. Press q again to quit.");
                app.quit_confirm = true;
            } else {
                app.running = false;
            }
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => app.running = false,
        KeyCode::Char('i')
            if matches!(
                app.view,
                app::View::Thread(_) | app::View::Channel(_) | app::View::Group(_)
            ) =>
        {
            app.load_draft();
            app.scroll_offset = 0;
            app.mode = Mode::Insert;
        }
        KeyCode::Char('m') => {
            if app.sending {
                app.set_error("Still sending previous message");
                return;
            }
            app.input.clear();
            app.cursor_pos = 0;
            app.mode = Mode::Message;
        }
        KeyCode::Char('n') => {
            app.input.clear();
            app.cursor_pos = 0;
            app.mode = Mode::Compose;
        }
        KeyCode::Char('c') => {
            app.channel_dir_cursor = 0;
            app.channel_dir_input.clear();
            app.scroll_offset = 0;
            app.view = app::View::ChannelDir;
        }
        KeyCode::Char('g') => {
            if app.sending {
                app.set_error("Still sending previous message");
                return;
            }
            app.input.clear();
            app.cursor_pos = 0;
            app.contact_idx = 0;
            app.pending_group_members.clear();
            // Always include self
            let my_pk = app.session.pubkey();
            let my_ss58 = app.session.my_ss58.clone();
            app.pending_group_members.push((my_pk, my_ss58));
            app.mode = Mode::CreateGroupMembers;
        }
        KeyCode::Char('r') => {
            // Fetch DAG gaps from chain
            let refs = match app.view {
                app::View::Thread(idx) => app.session.threads.get(idx).map(|c| c.gap_refs()),
                app::View::Channel(idx) => app.session.channels.get(idx).map(|c| c.gap_refs()),
                app::View::Group(idx) => app.session.groups.get(idx).map(|g| g.gap_refs()),
                _ => None,
            };
            if let Some(refs) = refs {
                for block_ref in refs {
                    let _ = send_tx.send(event::Event::FetchBlock { block_ref });
                }
            }
            // Also fetch from mirror if viewing a channel and mirror is configured
            if app.session.has_mirror
                && let app::View::Channel(idx) = app.view
                && let Some(ch) = app.session.channels.get(idx)
            {
                let _ = send_tx.send(event::Event::FetchChannelMirror {
                    channel_ref: ch.channel_ref,
                });
            }
        }
        KeyCode::Char('/') => {
            app.search_query.clear();
            app.input.clear();
            app.cursor_pos = 0;
            app.mode = Mode::Search;
        }
        KeyCode::Char('y') if app.view != app::View::ChannelDir => {
            let senders = app.build_picker_senders();
            if !senders.is_empty() {
                app.picker_senders = senders;
                app.contact_idx = 0;
                app.mode = Mode::SenderPicker;
            }
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.scroll_offset = app.scroll_offset.saturating_add(10);
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.scroll_offset = app.scroll_offset.saturating_sub(10);
        }
        KeyCode::PageUp => {
            app.scroll_offset = app.scroll_offset.saturating_add(20);
        }
        KeyCode::PageDown => {
            app.scroll_offset = app.scroll_offset.saturating_sub(20);
        }
        KeyCode::Home => app.scroll_offset = usize::MAX, // scroll to top
        KeyCode::Char('G') | KeyCode::End => app.scroll_offset = 0, // scroll to bottom
        KeyCode::Char(' ') => app.show_sidebar = !app.show_sidebar,
        KeyCode::Char('j') | KeyCode::Tab | KeyCode::Down => app.next_sidebar(),
        KeyCode::Char('k') | KeyCode::BackTab | KeyCode::Up => app.prev_sidebar(),
        _ => {}
    }
}

fn handle_insert_key(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    send_tx: &std::sync::mpsc::Sender<event::Event>,
    rt: &tokio::runtime::Runtime,
) {
    match key.code {
        KeyCode::Esc => {
            if app.msg_recipient.is_some() {
                app.clear_standalone();
                app.input.clear();
                app.cursor_pos = 0;
                app.set_status("Cancelled");
            } else if !app.input.is_empty() {
                app.save_draft();
                app.set_status("Draft saved");
            }
            app.mode = Mode::Normal;
        }
        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if !app.input.is_empty() {
                app.input.insert(app.cursor_pos, '\n');
                app.cursor_pos += 1;
            }
        }
        KeyCode::Up => {
            // Move cursor to same column on previous line
            let before = &app.input[..app.cursor_pos];
            if let Some(nl) = before.rfind('\n') {
                let col = app.cursor_pos - nl - 1;
                let prev_start = before[..nl].rfind('\n').map_or(0, |p| p + 1);
                let prev_len = nl - prev_start;
                app.cursor_pos = prev_start + col.min(prev_len);
            }
        }
        KeyCode::Down => {
            // Move cursor to same column on next line
            let before = &app.input[..app.cursor_pos];
            let line_start = before.rfind('\n').map_or(0, |p| p + 1);
            let col = app.cursor_pos - line_start;
            if let Some(nl) = app.input[app.cursor_pos..].find('\n') {
                let next_start = app.cursor_pos + nl + 1;
                let next_end = app.input[next_start..]
                    .find('\n')
                    .map_or(app.input.len(), |p| next_start + p);
                let next_len = next_end - next_start;
                app.cursor_pos = next_start + col.min(next_len);
            }
        }
        KeyCode::Enter => {
            if app.sending {
                app.set_error("Still sending previous message");
                return;
            }
            {
                let text = app.input.clone();
                match build_send_remark(app, &text) {
                    Ok(remark) => {
                        app.pending_remark = Some(remark.clone());
                        app.pending_text = Some(text);
                        app.pending_fee = None;
                        app.mode = Mode::Confirm;

                        let kp = app.session.keypair.clone();
                        let ss58 = app.session.my_ss58.clone();
                        let ci = app.session.chain_info.clone();
                        let url = app.session.node_url.clone();
                        let tx = send_tx.clone();
                        let symbol = app.session.token_symbol.clone();
                        let decimals = app.session.token_decimals;
                        rt.spawn(async move {
                            match extrinsic::estimate_fee(&url, &remark, &kp, &ss58, &ci).await {
                                Ok(fee) => {
                                    let display = util::format_fee(fee, decimals, &symbol);
                                    let _ = tx.send(event::Event::FeeEstimated {
                                        fee_display: display,
                                        fee_raw: Some(fee),
                                    });
                                }
                                Err(e) => {
                                    let _ = tx.send(event::Event::FeeEstimated {
                                        fee_display: format!("error: {e}"),
                                        fee_raw: None,
                                    });
                                }
                            }
                        });
                    }
                    Err(reason) => {
                        app.set_error(format!("Cannot send: {reason}"));
                    }
                }
            }
        }
        _ => {
            handle_text_input(app, key);
        }
    }
}

/// Handle Tab/BackTab for contact cycling in address input modes.
/// Returns true if the key was handled.
/// Get the selected contact's full SS58 address, or the raw input if no contact selected.
fn resolve_address_input(app: &App) -> String {
    let contacts = app.filtered_contacts();
    if !contacts.is_empty() && !app.input.is_empty() {
        let idx = app.contact_idx % contacts.len();
        let (ss58_short, pubkey) = &contacts[idx];
        let _ = ss58_short; // use the full address
        util::ss58_from_pubkey(pubkey)
    } else {
        app.input.trim().to_string()
    }
}

fn handle_compose_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down if app.input.is_empty() => {
            let count = app.filtered_contacts().len();
            if count > 0 {
                app.contact_idx = (app.contact_idx + 1).min(count - 1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up if app.input.is_empty() => {
            app.contact_idx = app.contact_idx.saturating_sub(1);
        }
        KeyCode::Enter => {
            let input = resolve_address_input(app);
            if input.is_empty() {
                app.set_error("Select a contact or paste an address");
                return;
            }
            match util::ss58_decode(&input) {
                Ok(pubkey) => {
                    if pubkey == app.session.pubkey() {
                        app.set_error("Cannot message yourself");
                        return;
                    }
                    let ss58 = util::ss58_short(&pubkey);
                    app.msg_recipient = Some((pubkey, ss58));
                    app.input.clear();
                    app.cursor_pos = 0;
                    app.mode = Mode::Insert;
                }
                Err(e) => {
                    app.set_error(format!("Invalid address: {e}"));
                }
            }
        }
        KeyCode::Esc => {
            if app.input.is_empty() {
                app.contact_idx = 0;
                app.mode = Mode::Normal;
            } else {
                app.input.clear();
                app.cursor_pos = 0;
            }
        }
        KeyCode::Backspace => {
            app.input.pop();
            app.cursor_pos = app.input.len();
        }
        KeyCode::Char(c) => {
            app.input.push(c);
            app.cursor_pos = app.input.len();
            app.contact_idx = 0;
        }
        _ => {}
    }
}

fn handle_message_key(app: &mut App, key: crossterm::event::KeyEvent) {
    if app.msg_recipient.is_none() {
        // Phase 1: address input (same UX as compose)
        match key.code {
            KeyCode::Char('j') | KeyCode::Down if app.input.is_empty() => {
                let count = app.filtered_contacts().len();
                if count > 0 {
                    app.contact_idx = (app.contact_idx + 1).min(count - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up if app.input.is_empty() => {
                app.contact_idx = app.contact_idx.saturating_sub(1);
            }
            KeyCode::Enter => {
                let input = resolve_address_input(app);
                if input.is_empty() {
                    app.set_error("Select a contact or paste an address");
                    return;
                }
                match util::ss58_decode(&input) {
                    Ok(pubkey) => {
                        if pubkey == app.session.pubkey() {
                            app.set_error("Cannot message yourself");
                            return;
                        }
                        let ss58 = util::ss58_short(&pubkey);
                        app.msg_recipient = Some((pubkey, ss58));
                        app.input.clear();
                        app.cursor_pos = 0;
                    }
                    Err(e) => {
                        app.set_error(format!("Invalid address: {e}"));
                    }
                }
            }
            KeyCode::Esc => {
                if app.input.is_empty() {
                    app.clear_standalone();
                    app.contact_idx = 0;
                    app.mode = Mode::Normal;
                } else {
                    app.input.clear();
                    app.cursor_pos = 0;
                }
            }
            KeyCode::Backspace => {
                app.input.pop();
                app.cursor_pos = app.input.len();
            }
            KeyCode::Char(c) => {
                app.input.push(c);
                app.cursor_pos = app.input.len();
                app.contact_idx = 0;
            }
            _ => {}
        }
    } else {
        // Phase 2: type selector
        match key.code {
            KeyCode::Char('p') => {
                app.msg_type = Some(0x01);
                app.mode = Mode::Insert;
            }
            KeyCode::Char('e') => {
                app.msg_type = Some(0x02);
                app.mode = Mode::Insert;
            }
            KeyCode::Esc => {
                app.clear_standalone();
                app.input.clear();
                app.cursor_pos = 0;
                app.mode = Mode::Normal;
            }
            _ => {}
        }
    }
}

fn handle_create_channel_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Enter => {
            let name = app.input.trim().to_string();
            if name.is_empty() {
                app.set_error("Channel name required");
                return;
            }
            if name.len() > samp::CHANNEL_NAME_MAX {
                app.set_error(format!(
                    "Channel name too long (max {} characters)",
                    samp::CHANNEL_NAME_MAX
                ));
                return;
            }
            app.pending_channel_name = Some(name);
            app.input.clear();
            app.cursor_pos = 0;
            app.mode = Mode::CreateChannelDesc;
        }
        KeyCode::Esc => {
            app.input.clear();
            app.cursor_pos = 0;
            app.pending_channel_name = None;
            app.mode = Mode::Normal;
        }
        _ => {
            handle_text_input(app, key);
        }
    }
}

fn handle_create_channel_desc_key(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    send_tx: &std::sync::mpsc::Sender<event::Event>,
    rt: &tokio::runtime::Runtime,
) {
    match key.code {
        KeyCode::Enter => {
            let desc = app.input.trim().to_string();
            if desc.len() > samp::CHANNEL_DESC_MAX {
                app.set_error(format!(
                    "Description too long (max {} characters)",
                    samp::CHANNEL_DESC_MAX
                ));
                return;
            }
            let name = match &app.pending_channel_name {
                Some(n) => n.clone(),
                None => {
                    app.mode = Mode::Normal;
                    return;
                }
            };
            app.pending_channel_desc = Some(desc.clone());
            match app.session.build_channel_create(&name, &desc) {
                Ok(remark) => {
                    app.pending_remark = Some(remark.clone());
                    app.pending_text = None;
                    app.pending_fee = None;
                    app.mode = Mode::Confirm;

                    let kp = app.session.keypair.clone();
                    let ss58 = app.session.my_ss58.clone();
                    let ci = app.session.chain_info.clone();
                    let url = app.session.node_url.clone();
                    let tx = send_tx.clone();
                    let symbol = app.session.token_symbol.clone();
                    let decimals = app.session.token_decimals;
                    rt.spawn(async move {
                        match extrinsic::estimate_fee(&url, &remark, &kp, &ss58, &ci).await {
                            Ok(fee) => {
                                let display = util::format_fee(fee, decimals, &symbol);
                                let _ = tx.send(event::Event::FeeEstimated {
                                    fee_display: display,
                                    fee_raw: Some(fee),
                                });
                            }
                            Err(e) => {
                                let _ = tx.send(event::Event::FeeEstimated {
                                    fee_display: format!("error: {e}"),
                                    fee_raw: None,
                                });
                            }
                        }
                    });
                }
                Err(reason) => {
                    app.set_error(format!("Cannot create channel: {reason}"));
                }
            }
        }
        KeyCode::Esc => {
            // Step back to name input
            app.input = app.pending_channel_name.take().unwrap_or_default();
            app.cursor_pos = app.input.len();
            app.mode = Mode::CreateChannel;
        }
        _ => {
            handle_text_input(app, key);
        }
    }
}

fn handle_create_group_members_key(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    _send_tx: &std::sync::mpsc::Sender<event::Event>,
) {
    match key.code {
        KeyCode::Enter => {
            if app.input.is_empty() {
                // Toggle contact under cursor
                let contacts = app.filtered_contacts();
                if let Some((ss58, pk)) = contacts.get(app.contact_idx % contacts.len().max(1)) {
                    let pk = *pk;
                    let ss58 = ss58.clone();
                    if let Some(pos) = app.pending_group_members.iter().position(|(k, _)| *k == pk)
                    {
                        if pk != app.session.pubkey() {
                            app.pending_group_members.remove(pos);
                        }
                    } else {
                        app.pending_group_members.push((pk, ss58));
                    }
                }
            } else {
                // Add matched contact or parse SS58 address
                let input = app.input.trim().to_string();
                let contacts = app.filtered_contacts();
                if let Some((ss58, pk)) = contacts.get(app.contact_idx % contacts.len().max(1)) {
                    let pk = *pk;
                    let ss58 = ss58.clone();
                    if !app.pending_group_members.iter().any(|(k, _)| *k == pk) {
                        app.pending_group_members.push((pk, ss58));
                    }
                    app.input.clear();
                    app.cursor_pos = 0;
                    app.contact_idx = 0;
                } else if input.len() >= 46 {
                    if let Some(pk) = util::pubkey_from_ss58(&input) {
                        if pk == app.session.pubkey() {
                            app.set_error("Already included (you)");
                        } else if app.pending_group_members.iter().any(|(k, _)| *k == pk) {
                            app.set_error("Already added");
                        } else {
                            let short = util::ss58_short(&pk);
                            app.pending_group_members.push((pk, short.clone()));
                            app.session.peer_pubkeys.insert(short.clone(), pk);
                            app.session.db.upsert_peer(&short, &pk);
                        }
                        app.input.clear();
                        app.cursor_pos = 0;
                        app.contact_idx = 0;
                    } else {
                        app.set_error("Invalid address");
                    }
                } else {
                    app.set_error("No match");
                }
            }
        }
        KeyCode::Tab => {
            if app.pending_group_members.len() < 2 {
                app.set_error("Add at least 1 other member");
                return;
            }
            let creator_pubkey = app.session.pubkey();
            let members: Vec<types::Pubkey> = app
                .pending_group_members
                .iter()
                .map(|(pk, _)| *pk)
                .collect();
            let idx = app.session.create_pending_group(creator_pubkey, members);
            app.view = app::View::Group(idx);
            app.input.clear();
            app.cursor_pos = 0;
            app.scroll_offset = 0;
            app.mode = Mode::Insert;
        }
        KeyCode::Down => {
            let len = app.filtered_contacts().len();
            if len > 0 {
                app.contact_idx = (app.contact_idx + 1) % len;
            }
        }
        KeyCode::Up => {
            let len = app.filtered_contacts().len();
            if len > 0 {
                app.contact_idx = if app.contact_idx == 0 {
                    len - 1
                } else {
                    app.contact_idx - 1
                };
            }
        }
        KeyCode::Esc => {
            if !app.input.is_empty() {
                app.input.clear();
                app.cursor_pos = 0;
                app.contact_idx = 0;
            } else {
                app.pending_group_members.clear();
                app.mode = Mode::Normal;
            }
        }
        _ => {
            if handle_text_input(app, key) {
                app.contact_idx = 0;
            }
        }
    }
}

fn handle_search_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.search_query.clear();
            app.mode = Mode::Normal;
        }
        KeyCode::Enter => {
            // Keep search active, go back to Normal with results highlighted
            app.search_query = app.input.clone();
            app.mode = Mode::Normal;
        }
        _ => {
            if handle_text_input(app, key) {
                app.search_query = app.input.clone();
            }
        }
    }
}

fn handle_sender_picker_key(app: &mut App, key: crossterm::event::KeyEvent) {
    let len = app.picker_senders.len();
    match key.code {
        KeyCode::Esc => {
            app.picker_senders.clear();
            app.mode = Mode::Normal;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if len > 0 {
                app.contact_idx = if app.contact_idx == 0 {
                    len - 1
                } else {
                    app.contact_idx - 1
                };
            }
        }
        KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
            if len > 0 {
                app.contact_idx = (app.contact_idx + 1) % len;
            }
        }
        KeyCode::Enter => {
            if let Some((short, pk)) = app.picker_senders.get(app.contact_idx).cloned() {
                copy_sender(app, &short, pk.as_ref());
            }
            app.picker_senders.clear();
            app.mode = Mode::Normal;
        }
        _ => {}
    }
}

fn copy_sender(app: &mut App, short_ss58: &str, pubkey: Option<&types::Pubkey>) {
    match pubkey {
        Some(pk) => {
            let full = util::ss58_from_pubkey(pk);
            util::copy_to_clipboard(&full);
            app.set_status(format!("Copied {short_ss58} to clipboard"));
        }
        None => {
            app.set_error(format!("{short_ss58}: full SS58 unavailable"));
        }
    }
}

fn parse_channel_ref(input: &str) -> Result<types::BlockRef, String> {
    let parts: Vec<&str> = input.split(':').collect();
    if parts.len() != 2 {
        return Err("expected block:index format".into());
    }
    let block: u32 = parts[0].parse().map_err(|_| "invalid block number")?;
    let index: u16 = parts[1].parse().map_err(|_| "invalid index")?;
    Ok(types::BlockRef { block, index })
}

fn handle_confirm_key(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    send_tx: &std::sync::mpsc::Sender<event::Event>,
) {
    match key.code {
        KeyCode::Enter => {
            if let Some(remark) = app.pending_remark.take() {
                let _ = send_tx.send(event::Event::SubmitRemark { remark });
                app.sending = true;
                if let (Some((pubkey, _)), None) = (&app.msg_recipient, app.msg_type) {
                    // New thread: create the thread object NOW (on submit)
                    let pubkey = *pubkey;
                    match app.session.create_thread(pubkey) {
                        Ok(idx) => {
                            app.pending_view = Some(app::View::Thread(idx));
                            app.view = app::View::Thread(idx);
                        }
                        Err(_) => {
                            app.pending_view = None;
                        }
                    }
                } else if app.msg_recipient.is_some() && app.msg_type.is_some() {
                    // Standalone message → pending shows in Outbox
                    app.pending_view = Some(app::View::Outbox);
                    app.view = app::View::Outbox;
                } else {
                    match app.view {
                        app::View::Thread(_) | app::View::Channel(_) | app::View::Group(_) => {
                            app.pending_view = Some(app.view);
                        }
                        _ => {
                            if let Some(name) = app.pending_channel_name.take() {
                                let creator_ss58 = util::ss58_short(&app.session.pubkey());
                                let idx = app.session.create_pending_channel(name, creator_ss58);
                                app.pending_view = Some(app::View::Channel(idx));
                                app.view = app::View::Channel(idx);
                            } else if app.is_pending_group() {
                                // Group was already created as pending when entering Insert mode
                                app.pending_view = Some(app.view);
                            } else {
                                app.pending_view = None;
                                app.pending_text = None;
                            }
                        }
                    }
                }
            }
            app.clear_standalone();
            // pending_view and pending_text are kept for the send-in-progress UI
            let view = app.pending_view;
            let text = app.pending_text.take();
            app.clear_pending();
            app.pending_view = view;
            app.pending_text = text;
            app.clear_draft();
            app.input.clear();
            app.cursor_pos = 0;
            app.mode = Mode::Normal;
        }
        KeyCode::Esc => {
            app.pending_remark = None;
            app.pending_fee = None;
            if app.is_pending_group() {
                // Group creation: step back to Insert mode
                if let Some(text) = app.pending_text.take() {
                    app.input = text;
                    app.cursor_pos = app.input.len();
                }
                app.mode = Mode::Insert;
            } else if app.is_pending_channel() {
                // Channel creation: step back to description
                app.input = app.pending_channel_desc.take().unwrap_or_default();
                app.cursor_pos = app.input.len();
                app.pending_text = None;
                app.mode = Mode::CreateChannelDesc;
            } else {
                // Message: step back to editing
                if let Some(text) = app.pending_text.take() {
                    app.input = text;
                    app.cursor_pos = app.input.len();
                }
                app.mode = Mode::Insert;
            }
        }
        _ => {}
    }
}
