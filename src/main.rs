mod app;
mod cli_fmt;
mod cmd;
mod ui;

use taolk::conversation::Conversation;
use taolk::{
    audio, chain, config, conversation, db, error, event, extrinsic, mirror, reader, session,
    types, util, wallet,
};

use app::{App, Mode};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
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
use std::io::stdout;
use std::sync::mpsc;
use std::time::Duration;

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
    Wallet {
        #[command(subcommand)]
        action: cmd::wallet::WalletAction,
    },
    Config {
        #[command(subcommand)]
        action: cmd::config::ConfigAction,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let cfg = config::load();

    match cli.command {
        Some(Commands::Wallet { action }) => cmd::wallet::run(action),
        Some(Commands::Config { action }) => cmd::config::run(action),
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
    let mut force_picker = false;

    loop {
        let result = if first_login || force_picker {
            run_lock_screen(&mut terminal, &events, wallets, preselected)?
        } else {
            run_lock_screen(&mut terminal, &events, &[], Some(&current_wallet))?
        };

        let (wallet_name, seed) = match result {
            Some(r) => r,
            None => break,
        };

        first_login = false;
        force_picker = false;
        current_wallet = wallet_name.clone();

        let exit = run_session(
            &mut terminal,
            &events,
            seed.as_bytes(),
            &wallet_name,
            node_url,
            mirror_urls,
            cfg,
        )?;
        drop(seed);
        match exit {
            SessionExit::Quit => break,
            SessionExit::Lock => {}
            SessionExit::SwitchWallet => {
                force_picker = true;
            }
        }
    }

    if cfg.ui.mouse {
        stdout().execute(DisableMouseCapture)?;
    }
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

enum SessionExit {
    Quit,
    Lock,
    SwitchWallet,
}

type UnlockResult = Option<(String, taolk::secret::Seed)>;

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
            use crate::ui::modal::{centered_line, centered_spans, horizontal_pad, vertical_pad};

            let area = frame.area();
            let w = area.width;

            let content_height = 18;
            let top_pad = vertical_pad(content_height, area.height);

            let mut lines: Vec<Line> = Vec::new();
            for _ in 0..top_pad {
                lines.push(Line::raw(""));
            }

            let logo_pad = horizontal_pad(55, w);
            for logo_line in LOGO {
                lines.push(Line::styled(
                    format!("{logo_pad}{logo_line}"),
                    Style::default().fg(Color::Cyan),
                ));
            }

            lines.push(Line::raw(""));
            lines.push(centered_line(
                SUBTITLE,
                w,
                Style::default().fg(Color::DarkGray),
            ));
            lines.push(Line::raw(""));
            lines.push(Line::raw(""));

            if show_carousel && !inserting {
                let win_start = wallet_idx
                    .saturating_sub(1)
                    .min(wallets.len().saturating_sub(3));
                let win_end = (win_start + 3).min(wallets.len());

                let mut spans: Vec<Span<'static>> = Vec::new();
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

                lines.push(centered_spans(spans, w));
            } else {
                lines.push(centered_line(
                    &current_wallet,
                    w,
                    Style::default().fg(Color::White),
                ));
            }

            lines.push(Line::raw(""));

            let prompt = "Password: ";
            let prompt_color = if inserting {
                Color::White
            } else {
                Color::DarkGray
            };
            let pp_str = horizontal_pad(prompt.len(), w);
            let prompt_x_offset = pp_str.len();
            lines.push(Line::from(vec![
                Span::raw(pp_str),
                Span::styled(prompt, Style::default().fg(prompt_color)),
            ]));

            if let Some(err) = &error_msg {
                lines.push(Line::raw(""));
                lines.push(centered_line(err, w, Style::default().fg(Color::Red)));
            } else {
                lines.push(Line::raw(""));
                lines.push(Line::raw(""));
            }

            let hints = if inserting {
                "Enter unlock \u{00B7} Esc back"
            } else if show_carousel {
                "\u{2190}/\u{2192} select \u{00B7} i unlock \u{00B7} q quit"
            } else {
                "i unlock \u{00B7} q quit"
            };
            lines.push(centered_line(
                hints,
                w,
                Style::default().fg(Color::DarkGray),
            ));

            frame.render_widget(Paragraph::new(lines), area);

            if inserting {
                let cursor_y =
                    area.y + u16::try_from(top_pad).unwrap_or(u16::MAX) + 7 + 1 + 1 + 2 + 1 + 1;
                let cursor_x = area.x
                    + u16::try_from(prompt_x_offset).unwrap_or(u16::MAX)
                    + u16::try_from(prompt.len()).unwrap_or(u16::MAX);
                if cursor_x < area.x + area.width && cursor_y < area.y + area.height {
                    frame.set_cursor_position((cursor_x, cursor_y));
                }
            }
        })?;

        match events.next()? {
            TuiEvent::Key(key) if inserting => match key.code {
                KeyCode::Enter => match wallet::open(
                    &current_wallet,
                    &taolk::secret::Password::new((*password).clone()),
                ) {
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

/// Returns a usable seed for one signing/encryption operation. In normal mode this hands
/// back the cached session seed. In ephemeral mode (`security.require_password_per_send`),
/// this opens a password prompt modal and returns the freshly-derived seed; the caller is
/// responsible for dropping it as soon as the operation completes.
fn acquire_seed(
    app: &App,
    wallet_name: &str,
    require_password: bool,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    events: &TuiEventHandler,
) -> Result<Option<zeroize::Zeroizing<[u8; 32]>>, Box<dyn std::error::Error>> {
    if !require_password {
        return Ok(app
            .session
            .cached_seed()
            .map(|s| zeroize::Zeroizing::new(*s)));
    }
    Ok(prompt_password_modal(terminal, events, wallet_name)?
        .map(|seed| zeroize::Zeroizing::new(*seed.as_bytes())))
}

fn dispatch_unlock_all(
    app: &mut App,
    wallet_name: &str,
    require_password: bool,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    events: &TuiEventHandler,
    send_tx: &std::sync::mpsc::Sender<event::Event>,
) -> Result<(), Box<dyn std::error::Error>> {
    let seed = match acquire_seed(app, wallet_name, require_password, terminal, events)? {
        Some(s) => s,
        None => {
            app.set_status("Cancelled");
            return Ok(());
        }
    };
    let my_pubkey = app.session.pubkey();
    let view_scalar = app.session.view_scalar();
    let keys = taolk::secret::DecryptionKeys::new(view_scalar.to_bytes(), Some(*seed));
    let pending: Vec<app::LockedOutbound> = std::mem::take(&mut app.locked_outbound);
    let mut unlocked = 0usize;
    for entry in pending {
        let Ok(remark) = samp::decode_remark(&entry.remark_bytes) else {
            continue;
        };
        let source = reader::RemarkSource {
            sender: entry.sender,
            remark,
            remark_bytes: entry.remark_bytes,
            block: types::BlockRef {
                block: entry.block_number,
                index: entry.ext_index,
            },
            timestamp_secs: entry.timestamp,
        };
        reader::process_remark(&source, &my_pubkey, &keys, send_tx);
        unlocked += 1;
    }
    drop(seed);
    app.set_status(format!("Unlocked {unlocked} message(s)"));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn dispatch_pending_send(
    app: &mut App,
    text: String,
    wallet_name: &str,
    require_password: bool,
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    events: &TuiEventHandler,
    send_tx: &std::sync::mpsc::Sender<event::Event>,
    rt: &tokio::runtime::Runtime,
) -> Result<(), Box<dyn std::error::Error>> {
    let seed = match acquire_seed(app, wallet_name, require_password, terminal, events)? {
        Some(s) => s,
        None => {
            app.set_status("Cancelled");
            return Ok(());
        }
    };
    let result = build_send_remark(app, &seed, &text);
    drop(seed);
    match result {
        Ok(remark) => {
            app.pending_remark = Some(remark.clone());
            app.pending_text = Some(text);
            app.pending_fee = None;
            app.mode = Mode::Confirm;

            let signing = app.session.signing();
            let ss58 = app.session.my_ss58.clone();
            let ci = app.session.chain_info.clone();
            let url = app.session.node_url.clone();
            let tx = send_tx.clone();
            let symbol = app.session.token_symbol.clone();
            let decimals = app.session.token_decimals;
            rt.spawn(async move {
                match extrinsic::estimate_fee(&url, &remark, &signing, &ss58, &ci).await {
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
    Ok(())
}

fn prompt_password_modal(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    events: &TuiEventHandler,
    wallet_name: &str,
) -> Result<Option<taolk::secret::Seed>, Box<dyn std::error::Error>> {
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, Clear, Paragraph};

    let mut password = zeroize::Zeroizing::new(String::new());
    let mut error_msg: Option<String> = None;

    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            let want_w = 48.min(area.width);
            let want_h = 7u16.min(area.height);
            let rect = crate::ui::modal::centered_rect(area, want_w, want_h);

            frame.render_widget(Clear, rect);
            let block = Block::default()
                .title(Span::styled(
                    format!(" Confirm password — {wallet_name} "),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan));
            let inner = block.inner(rect);
            frame.render_widget(block, rect);

            let mut lines: Vec<Line> = Vec::new();
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("Password: ", Style::default().fg(Color::White)),
            ]));
            if let Some(err) = &error_msg {
                lines.push(Line::raw(""));
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(err.clone(), Style::default().fg(Color::Red)),
                ]));
            } else {
                lines.push(Line::raw(""));
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        "Enter sign \u{00B7} Esc cancel",
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }
            frame.render_widget(Paragraph::new(lines), inner);

            let cursor_x = inner.x + 2 + "Password: ".len() as u16;
            let cursor_y = inner.y + 1;
            if cursor_x < inner.x + inner.width && cursor_y < inner.y + inner.height {
                frame.set_cursor_position(Rect {
                    x: cursor_x,
                    y: cursor_y,
                    width: 1,
                    height: 1,
                });
            }
        })?;

        match events.next()? {
            TuiEvent::Key(key) => match key.code {
                KeyCode::Enter => match wallet::open(
                    wallet_name,
                    &taolk::secret::Password::new((*password).clone()),
                ) {
                    Ok(seed) => {
                        password.clear();
                        return Ok(Some(seed));
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
                    password.clear();
                    return Ok(None);
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
            TuiEvent::Tick | TuiEvent::Core(_) | TuiEvent::Mouse(_) => {}
        }
    }
}

fn run_session(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    events: &TuiEventHandler,
    seed: &[u8; 32],
    wallet_name: &str,
    node_url: &str,
    mirror_urls: &[String],
    cfg: &config::Config,
) -> Result<SessionExit, Box<dyn std::error::Error>> {
    let signing = taolk::secret::Seed::from_bytes(*seed).derive_signing_key();
    let my_pubkey = signing.public_key();

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

    let db = db::Db::open(
        wallet_name,
        seed,
        chain_info.chain_params.genesis_hash.as_bytes(),
    )?;
    let keep_seed = !cfg.security.require_password_per_send;
    let session = session::Session::new(
        signing,
        zeroize::Zeroizing::new(*seed),
        keep_seed,
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
        &chain_info.account_storage,
    )) {
        app.session.balance = Some(bal);
    }

    let event_tx = events.core_sender();
    let lock_timeout = std::time::Duration::from_secs(cfg.security.lock_timeout);
    let mut last_activity = std::time::Instant::now();

    {
        let url = node_url.to_string();
        let tx = event_tx.clone();
        let keys = app.session.decryption_keys();
        rt.spawn(async move {
            let _ = tx.send(event::Event::Status("Connected".into()));
            chain::subscribe_blocks(&url, my_pubkey, keys, tx).await;
        });
    }

    app.session.has_mirror = !mirror_urls.is_empty();
    if app.session.has_mirror {
        let subscribed: Vec<types::BlockRef> =
            app.session.channels.iter().map(|c| c.channel_ref).collect();
        let urls: Vec<String> = mirror_urls.iter().map(|u| u.to_string()).collect();
        let node = node_url.to_string();
        let keys = app.session.decryption_keys();
        let pubkey = my_pubkey;
        let tx = event_tx.clone();
        let chain_name = chain_info.name.clone();
        let ss58_prefix = chain_info.ss58_prefix;
        rt.spawn(async move {
            mirror::sync(
                urls,
                &node,
                &chain_name,
                ss58_prefix,
                &keys,
                &pubkey,
                subscribed,
                0,
                tx,
            )
            .await;
        });
    } else {
        app.sound_armed = true;
    }

    app.set_status("Connected");

    while app.running {
        if let Some(text) = app.pending_send_text.take() {
            dispatch_pending_send(
                &mut app,
                text,
                wallet_name,
                cfg.security.require_password_per_send,
                terminal,
                events,
                &event_tx,
                &rt,
            )?;
        }
        if app.pending_unlock_all {
            app.pending_unlock_all = false;
            dispatch_unlock_all(
                &mut app,
                wallet_name,
                cfg.security.require_password_per_send,
                terminal,
                events,
                &event_tx,
            )?;
        }

        terminal.draw(|frame| ui::render(frame, &app))?;

        if cfg.security.lock_timeout > 0 && last_activity.elapsed() > lock_timeout {
            return Ok(SessionExit::Lock);
        }

        match events.next()? {
            TuiEvent::Key(key) => {
                last_activity = std::time::Instant::now();
                if (key.code == KeyCode::Char('l') && key.modifiers.contains(KeyModifiers::CONTROL))
                    || key.code == KeyCode::Char('\x0c')
                {
                    return Ok(SessionExit::Lock);
                }
                if key.code == KeyCode::Char('w') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    return Ok(SessionExit::SwitchWallet);
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
                    let layout = app.session.chain_info.account_storage.clone();
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
                let ts = DateTime::<Utc>::from_timestamp(i64::try_from(timestamp).unwrap_or(0), 0)
                    .unwrap_or_default();
                let sender_ss58 = util::ss58_short(&sender);
                let is_mine = sender == app.session.pubkey();
                let kind = ct & 0x0F;

                match kind {
                    0x00 | 0x01 => {
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
                let ts = DateTime::<Utc>::from_timestamp(i64::try_from(timestamp).unwrap_or(0), 0)
                    .unwrap_or_default();
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
                let ts = DateTime::<Utc>::from_timestamp(i64::try_from(timestamp).unwrap_or(0), 0)
                    .unwrap_or_default();
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
                let signing = app.session.signing();
                let ss58 = app.session.my_ss58.clone();
                let ci = chain_info.clone();
                let tx = event_tx.clone();
                rt.spawn(async move {
                    match extrinsic::submit_remark(&url, &remark, &signing, &ss58, &ci).await {
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
                if !mirror_urls.is_empty() {
                    app.set_status("Loading...");
                    let urls: Vec<String> = mirror_urls.iter().map(|u| u.to_string()).collect();
                    let node = node_url.to_string();
                    let tx = event_tx.clone();
                    let pk = my_pubkey;
                    let keys = app.session.decryption_keys();
                    let chain_name = chain_info.name.clone();
                    let ss58_prefix = chain_info.ss58_prefix;
                    rt.spawn(async move {
                        mirror::fetch_channel(
                            urls,
                            &node,
                            &chain_name,
                            ss58_prefix,
                            channel_ref,
                            &pk,
                            &keys,
                            tx,
                        )
                        .await;
                    });
                }
            }
            TuiEvent::Core(event::Event::FetchBlock { block_ref }) => {
                app.set_status("Loading...");
                let url = node_url.to_string();
                let tx = event_tx.clone();
                let keys = app.session.decryption_keys();
                rt.spawn(async move {
                    chain::fetch_and_process_extrinsic(
                        &url,
                        block_ref.block,
                        block_ref.index,
                        my_pubkey,
                        keys,
                        tx.clone(),
                    )
                    .await;
                    let _ = tx.send(event::Event::GapsRefreshed);
                });
            }
            TuiEvent::Core(event::Event::GapsRefreshed) => {
                for i in 0..app.session.threads.len() {
                    app.session.refresh_gaps(taolk::db::ConvKind::Thread, i);
                }
                for i in 0..app.session.channels.len() {
                    app.session.refresh_gaps(taolk::db::ConvKind::Channel, i);
                }
                for i in 0..app.session.groups.len() {
                    app.session.refresh_gaps(taolk::db::ConvKind::Group, i);
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
                let url = node_url.to_string();
                let pk = my_pubkey;
                let tx = event_tx.clone();
                let layout = app.session.chain_info.account_storage.clone();
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
            TuiEvent::Core(event::Event::ConnectionStatus(state)) => {
                app.connection = state;
            }
            TuiEvent::Core(event::Event::Status(msg)) => {
                app.set_status(msg);
            }
            TuiEvent::Core(event::Event::CatchupComplete) => {
                app.sound_armed = true;
                for i in 0..app.session.threads.len() {
                    app.session.refresh_gaps(taolk::db::ConvKind::Thread, i);
                }
                for i in 0..app.session.channels.len() {
                    app.session.refresh_gaps(taolk::db::ConvKind::Channel, i);
                }
                for i in 0..app.session.groups.len() {
                    app.session.refresh_gaps(taolk::db::ConvKind::Group, i);
                }
            }
            TuiEvent::Core(event::Event::LockedOutbound {
                sender,
                block_number,
                ext_index,
                timestamp,
                remark_bytes,
            }) => {
                if !app
                    .locked_outbound
                    .iter()
                    .any(|m| m.block_number == block_number && m.ext_index == ext_index)
                {
                    app.locked_outbound.push(app::LockedOutbound {
                        sender,
                        block_number,
                        ext_index,
                        timestamp,
                        remark_bytes,
                    });
                }
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
                app.set_chain_error(&e);
            }
        }
    }

    Ok(SessionExit::Quit)
}

fn build_send_remark(app: &App, seed: &[u8; 32], text: &str) -> error::Result<samp::RemarkBytes> {
    if let (Some((pubkey, _)), Some(ct)) = (&app.msg_recipient, app.msg_type) {
        return match ct {
            0x01 => app.session.build_public_message(pubkey, text),
            0x02 => app.session.build_encrypted_message(seed, pubkey, text),
            _ => Err(error::SdkError::Other("Invalid message type".into())),
        };
    }

    if let (Some((pubkey, _)), None) = (&app.msg_recipient, app.msg_type) {
        return app.session.build_thread_root(seed, pubkey, text);
    }

    match app.view {
        app::View::Thread(idx) => app.session.build_thread_reply(seed, idx, text),
        app::View::Channel(idx) => app.session.build_channel_message(idx, text),
        app::View::Group(idx) => {
            let group = app
                .session
                .groups
                .get(idx)
                .ok_or_else(|| error::SdkError::NotFound("No group selected".into()))?;
            if group.group_ref.is_zero() {
                app.session
                    .build_group_create(seed, &group.members.clone(), text)
            } else {
                app.session.build_group_message(seed, idx, text)
            }
        }
        _ => Err(error::SdkError::Other("Cannot send from this view".into())),
    }
}

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
                app.cursor_pos = app.input[..app.cursor_pos].rfind(' ').unwrap_or(0);
            } else {
                app.cursor_pos = app.cursor_pos.saturating_sub(1);
            }
        }
        KeyCode::Right => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
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
            let row = usize::from(y.saturating_sub(1));
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
        Mode::Insert => handle_insert_key(app, key),
        Mode::Confirm => handle_confirm_key(app, key, send_tx),
        Mode::Compose => handle_compose_key(app, key),
        Mode::Message => handle_message_key(app, key),
        Mode::CreateChannel => handle_create_channel_key(app, key),
        Mode::CreateChannelDesc => handle_create_channel_desc_key(app, key, send_tx, rt),
        Mode::CreateGroupMembers => handle_create_group_members_key(app, key, send_tx),
        Mode::Search => handle_search_key(app, key),
        Mode::SenderPicker => handle_sender_picker_key(app, key),
        Mode::Help => handle_help_key(app, key),
    }
}

fn handle_help_key(app: &mut App, _key: crossterm::event::KeyEvent) {
    app.mode = Mode::Normal;
}

fn handle_normal_key(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    send_tx: &std::sync::mpsc::Sender<event::Event>,
) {
    if key.code != KeyCode::Char('q') {
        app.quit_confirm = false;
    }
    if app.view == app::View::ChannelDir && handle_channel_dir_key(app, key, send_tx) {
        return;
    }
    handle_global_normal_key(app, key, send_tx);
}

/// View-local handler for the channel directory. Returns true if the key was
/// fully consumed (no fall-through to global). The contract:
///
/// - Input editing keys (digit/colon, Backspace, Enter, Esc) are always consumed.
/// - While the user is typing a channel ref input, every other non-modifier key
///   is dropped (consumed) so global keybinds can't fire under their fingers.
///   Modifier-keys (Ctrl-c, Ctrl-u, Ctrl-d, etc.) still fall through.
/// - When browsing (input empty), Up/Down move the channel cursor (consumed)
///   and `c` enters CreateChannel mode (consumed). Everything else (j/k for
///   sidebar nav, ?/q/m/n/g/r//, etc.) falls through to the global handler.
fn handle_channel_dir_key(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    send_tx: &std::sync::mpsc::Sender<event::Event>,
) -> bool {
    let typing = !app.channel_dir_input.is_empty();
    let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    // Always-consumed input editing keys.
    match key.code {
        KeyCode::Backspace => {
            app.channel_dir_input.pop();
            return true;
        }
        KeyCode::Char(c) if !has_ctrl && (c.is_ascii_digit() || c == ':') => {
            app.channel_dir_input.push(c);
            return true;
        }
        KeyCode::Enter => {
            if typing {
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
                            let _ = send_tx.send(event::Event::FetchChannelMirror { channel_ref });
                        }
                    }
                    Err(e) => {
                        app.set_error(format!("Invalid channel ref: {e}"));
                    }
                }
            } else if let Some(info) = app.session.known_channels.get(app.channel_dir_cursor) {
                let channel_ref = info.channel_ref;
                if app.session.is_subscribed(&channel_ref) {
                    if let Some(idx) = app.session.channel_idx(&channel_ref)
                        && let Some(name) = app.session.unsubscribe_channel(idx)
                    {
                        app.set_status(format!("Left #{name}"));
                    }
                } else {
                    let idx = app.session.subscribe_channel(channel_ref);
                    app.set_status(format!("Subscribed to #{}", app.session.channels[idx].name));
                    let _ = send_tx.send(event::Event::FetchBlock {
                        block_ref: channel_ref,
                    });
                    if app.session.has_mirror {
                        let _ = send_tx.send(event::Event::FetchChannelMirror { channel_ref });
                    }
                }
            }
            return true;
        }
        KeyCode::Esc => {
            if typing {
                app.channel_dir_input.clear();
            } else {
                app.view = app::View::Inbox;
            }
            return true;
        }
        _ => {}
    }

    // While typing channel ref input, drop every other non-modifier key so the
    // user's keystrokes can't accidentally fire global keybinds.
    if typing && !has_ctrl {
        return true;
    }

    // Browsing mode (or modifier-key passthrough): only a few view-internal
    // keys; everything else falls through to the global handler.
    match key.code {
        KeyCode::Up if !typing => {
            app.channel_dir_cursor = app.channel_dir_cursor.saturating_sub(1);
            true
        }
        KeyCode::Down if !typing => {
            if !app.session.known_channels.is_empty() {
                app.channel_dir_cursor =
                    (app.channel_dir_cursor + 1).min(app.session.known_channels.len() - 1);
            }
            true
        }
        KeyCode::Char('c') if !has_ctrl && !typing => {
            if !app.check_not_sending() {
                return true;
            }
            app.enter_mode(Mode::CreateChannel);
            true
        }
        _ => false,
    }
}

fn handle_global_normal_key(
    app: &mut App,
    key: crossterm::event::KeyEvent,
    send_tx: &std::sync::mpsc::Sender<event::Event>,
) {
    match key.code {
        KeyCode::Char('?') => {
            app.mode = Mode::Help;
        }
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
            if !app.check_not_sending() {
                return;
            }
            app.enter_mode(Mode::Message);
        }
        KeyCode::Char('n') => {
            app.enter_mode(Mode::Compose);
        }
        KeyCode::Char('c') => {
            app.channel_dir_cursor = 0;
            app.channel_dir_input.clear();
            app.scroll_offset = 0;
            app.view = app::View::ChannelDir;
        }
        KeyCode::Char('g') => {
            if !app.check_not_sending() {
                return;
            }
            app.reset_input();
            app.contact_idx = 0;
            app.pending_group_members.clear();
            let my_pk = app.session.pubkey();
            let my_ss58 = app.session.my_ss58.clone();
            app.pending_group_members.push((my_pk, my_ss58));
            app.mode = Mode::CreateGroupMembers;
        }
        KeyCode::Char('r') => {
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
            app.enter_mode(Mode::Search);
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
        KeyCode::Char('U') => {
            if app.locked_outbound.is_empty() {
                app.set_status("No locked messages");
            } else {
                app.pending_unlock_all = true;
            }
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
        KeyCode::Home => app.scroll_offset = usize::MAX,
        KeyCode::Char('G') | KeyCode::End => app.scroll_offset = 0,
        KeyCode::Char(' ') => app.show_sidebar = !app.show_sidebar,
        KeyCode::Char('j') | KeyCode::Tab | KeyCode::Down => app.next_sidebar(),
        KeyCode::Char('k') | KeyCode::BackTab | KeyCode::Up => app.prev_sidebar(),
        _ => {}
    }
}

fn handle_insert_key(app: &mut App, key: crossterm::event::KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            if app.msg_recipient.is_some() {
                app.clear_standalone();
                app.reset_input();
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
            let before = &app.input[..app.cursor_pos];
            if let Some(nl) = before.rfind('\n') {
                let col = app.cursor_pos - nl - 1;
                let prev_start = before[..nl].rfind('\n').map_or(0, |p| p + 1);
                let prev_len = nl - prev_start;
                app.cursor_pos = prev_start + col.min(prev_len);
            }
        }
        KeyCode::Down => {
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
            if !app.check_not_sending() {
                return;
            }
            // Defer encryption + remark build to the main loop, which has access to the
            // terminal/events needed for the password prompt in ephemeral mode.
            app.pending_send_text = Some(app.input.clone());
        }
        _ => {
            handle_text_input(app, key);
        }
    }
}

fn resolve_address_input(app: &App) -> String {
    let contacts = app.filtered_contacts();
    if !contacts.is_empty() && !app.input.is_empty() {
        let idx = app.contact_idx % contacts.len();
        let (_, pubkey) = &contacts[idx];
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
                    app.enter_mode(Mode::Insert);
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
                app.reset_input();
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
                        app.reset_input();
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
                    app.reset_input();
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
                app.enter_mode(Mode::Normal);
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
            app.enter_mode(Mode::CreateChannelDesc);
        }
        KeyCode::Esc => {
            app.reset_input();
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

                    let signing = app.session.signing();
                    let ss58 = app.session.my_ss58.clone();
                    let ci = app.session.chain_info.clone();
                    let url = app.session.node_url.clone();
                    let tx = send_tx.clone();
                    let symbol = app.session.token_symbol.clone();
                    let decimals = app.session.token_decimals;
                    rt.spawn(async move {
                        match extrinsic::estimate_fee(&url, &remark, &signing, &ss58, &ci).await {
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
                let contacts = app.filtered_contacts();
                if let Some((ss58, pk)) = contacts.get(app.contact_idx % contacts.len().max(1)) {
                    let pk = *pk;
                    let ss58 = ss58.clone();
                    if let Some(pos) = app.pending_group_members.iter().position(|(k, _)| *k == pk)
                    {
                        if pk != app.session.pubkey() {
                            app.pending_group_members.remove(pos);
                        }
                    } else if app.pending_group_members.len() >= session::MAX_GROUP_MEMBERS {
                        app.set_error(format!(
                            "Max {} members per group",
                            session::MAX_GROUP_MEMBERS
                        ));
                    } else {
                        app.pending_group_members.push((pk, ss58));
                    }
                }
            } else {
                let input = app.input.trim().to_string();
                let contacts = app.filtered_contacts();
                if let Some((ss58, pk)) = contacts.get(app.contact_idx % contacts.len().max(1)) {
                    let pk = *pk;
                    let ss58 = ss58.clone();
                    if app.pending_group_members.iter().any(|(k, _)| *k == pk) {
                    } else if app.pending_group_members.len() >= session::MAX_GROUP_MEMBERS {
                        app.set_error(format!(
                            "Max {} members per group",
                            session::MAX_GROUP_MEMBERS
                        ));
                    } else {
                        app.pending_group_members.push((pk, ss58));
                    }
                    app.reset_input();
                    app.contact_idx = 0;
                } else if input.len() >= 46 {
                    if let Some(pk) = util::pubkey_from_ss58(&input) {
                        if pk == app.session.pubkey() {
                            app.set_error("Already included (you)");
                        } else if app.pending_group_members.iter().any(|(k, _)| *k == pk) {
                            app.set_error("Already added");
                        } else if app.pending_group_members.len() >= session::MAX_GROUP_MEMBERS {
                            app.set_error(format!(
                                "Max {} members per group",
                                session::MAX_GROUP_MEMBERS
                            ));
                        } else {
                            let short = util::ss58_short(&pk);
                            app.pending_group_members.push((pk, short.clone()));
                            app.session.peer_pubkeys.insert(short.clone(), pk);
                            app.session.db.upsert_peer(&short, &pk);
                        }
                        app.reset_input();
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
            app.reset_input();
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
                app.reset_input();
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
            if util::copy_to_clipboard(&full) {
                app.set_status(format!("Copied {short_ss58} to clipboard"));
            } else {
                app.set_error(
                    "Clipboard unavailable — install xclip / wl-copy, or use a terminal that supports OSC 52",
                );
            }
        }
        None => {
            app.set_error(format!("{short_ss58}: full SS58 unavailable"));
        }
    }
}

fn parse_channel_ref(input: &str) -> Result<types::BlockRef, &'static str> {
    let parts: Vec<&str> = input.split(':').collect();
    if parts.len() != 2 {
        return Err("expected block:index format");
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
            if let (Some(balance), Some(fee)) = (app.session.balance, app.last_fee)
                && balance < fee
            {
                let symbol = app.session.token_symbol.clone();
                let decimals = app.session.token_decimals;
                app.set_error(format!(
                    "Insufficient balance: have {}, need {} for fee",
                    util::format_balance_short(balance, decimals, &symbol),
                    util::format_fee(fee, decimals, &symbol),
                ));
                app.mode = Mode::Insert;
                return;
            }
            if let Some(remark) = app.pending_remark.take() {
                let _ = send_tx.send(event::Event::SubmitRemark { remark });
                app.sending = true;
                if let (Some((pubkey, _)), None) = (&app.msg_recipient, app.msg_type) {
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
            let view = app.pending_view;
            let text = app.pending_text.take();
            app.clear_pending();
            app.pending_view = view;
            app.pending_text = text;
            app.clear_draft();
            app.enter_mode(Mode::Normal);
        }
        KeyCode::Esc => {
            app.pending_remark = None;
            app.pending_fee = None;
            if app.is_pending_group() {
                if let Some(text) = app.pending_text.take() {
                    app.input = text;
                    app.cursor_pos = app.input.len();
                }
                app.mode = Mode::Insert;
            } else if app.is_pending_channel() {
                app.input = app.pending_channel_desc.take().unwrap_or_default();
                app.cursor_pos = app.input.len();
                app.pending_text = None;
                app.mode = Mode::CreateChannelDesc;
            } else if let Some(text) = app.pending_text.take() {
                app.input = text;
                app.cursor_pos = app.input.len();
                app.mode = Mode::Insert;
            } else {
                app.mode = Mode::Insert;
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_clap_derive_is_well_formed() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parse_channel_ref_valid() {
        let r = parse_channel_ref("12345:7").unwrap();
        assert_eq!(r.block, 12345);
        assert_eq!(r.index, 7);
    }

    #[test]
    fn parse_channel_ref_zero() {
        let r = parse_channel_ref("0:0").unwrap();
        assert_eq!(r.block, 0);
        assert_eq!(r.index, 0);
    }

    #[test]
    fn parse_channel_ref_missing_colon() {
        assert!(parse_channel_ref("12345").is_err());
    }

    #[test]
    fn parse_channel_ref_too_many_colons() {
        assert!(parse_channel_ref("1:2:3").is_err());
    }

    #[test]
    fn parse_channel_ref_empty() {
        assert!(parse_channel_ref("").is_err());
    }

    #[test]
    fn parse_channel_ref_non_numeric_block() {
        assert!(parse_channel_ref("foo:0").is_err());
    }

    #[test]
    fn parse_channel_ref_non_numeric_index() {
        assert!(parse_channel_ref("0:bar").is_err());
    }

    #[test]
    fn parse_channel_ref_block_overflow() {
        let s = format!("{}:0", u64::MAX);
        assert!(parse_channel_ref(&s).is_err());
    }

    #[test]
    fn parse_channel_ref_index_overflow() {
        assert!(parse_channel_ref("100:99999").is_err());
    }
}
