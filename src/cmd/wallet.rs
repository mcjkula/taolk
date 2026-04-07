use clap::{ArgGroup, Subcommand};
use taolk::secret::{Password, Phrase, Seed};
use taolk::{util, wallet};
use zeroize::Zeroize;

use crate::cli_fmt::{
    BOLD, CYAN, DIM, RESET, YELLOW, blank, error, header, hint, label, label_magenta, success,
};

#[derive(Subcommand)]
pub enum WalletAction {
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

pub fn run(action: WalletAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        WalletAction::Create { name, password } => create(&name, password),
        WalletAction::Import {
            name,
            mnemonic,
            seed,
            password,
        } => import(&name, mnemonic, seed, password),
        WalletAction::List => list(),
    }
}

fn create(
    wallet_name: &str,
    cli_password: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
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
        Some(p) => Password::new(p),
        None => prompt_new_password()?,
    };

    let phrase = Phrase::generate()?;
    let seed = Seed::from_phrase(&phrase);
    let words: Vec<&str> = phrase.words().split_whitespace().collect();
    let canonical_phrase = words.join(" ");

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
    hint("  Type the recovery phrase to confirm (3 attempts):");
    blank();

    let mut attempts_left = 3u32;
    loop {
        let typed = rpassword::prompt_password(format!("  {YELLOW}Phrase:{RESET} "))?;
        if normalize_phrase(&typed) == normalize_phrase(&canonical_phrase) {
            success("Verified");
            break;
        }
        attempts_left -= 1;
        if attempts_left == 0 {
            error("Recovery phrase did not match after 3 attempts. Wallet not created.");
            hint("  Re-run `taolk wallet create` and copy the phrase carefully.");
            std::process::exit(1);
        }
        error(&format!(
            "Phrases don't match. {attempts_left} attempt{} left.",
            if attempts_left == 1 { "" } else { "s" }
        ));
    }

    wallet::create(wallet_name, &password, &seed)?;

    let address = util::ss58_from_pubkey(&seed.derive_signing_key().public_key());

    blank();
    success("Wallet created");
    blank();
    label("Wallet", &format!("{BOLD}{wallet_name}{RESET}"));
    label_magenta("Address", &address);
    blank();

    Ok(())
}

fn normalize_phrase(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
        .to_lowercase()
}

fn import(
    wallet_name: &str,
    mnemonic: Option<String>,
    seed_hex: Option<String>,
    cli_password: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if wallet::wallet_exists(wallet_name) {
        error(&format!("Wallet '{}' already exists", wallet_name));
        hint("  Use --wallet <other-name> to import under a different name");
        std::process::exit(1);
    }

    header(&format!(
        "\u{03C4}alk \u{2014} Import wallet '{wallet_name}'"
    ));
    blank();

    let seed = if let Some(phrase) = mnemonic {
        let p = Phrase::parse(&phrase)?;
        Seed::from_phrase(&p)
    } else if let Some(hex) = seed_hex {
        Seed::from_hex(&hex)?
    } else {
        error("Provide --mnemonic or --seed");
        std::process::exit(1);
    };

    let password = match cli_password {
        Some(p) => Password::new(p),
        None => prompt_new_password()?,
    };
    wallet::create(wallet_name, &password, &seed)?;

    let address = util::ss58_from_pubkey(&seed.derive_signing_key().public_key());

    blank();
    success("Wallet imported");
    blank();
    label("Wallet", &format!("{BOLD}{wallet_name}{RESET}"));
    label_magenta("Address", &address);
    blank();

    Ok(())
}

fn list() -> Result<(), Box<dyn std::error::Error>> {
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

pub fn prompt_new_password() -> Result<Password, Box<dyn std::error::Error>> {
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
    Ok(Password::new(password))
}
