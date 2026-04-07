use clap::Subcommand;
use taolk::config;

use crate::cli_fmt::{
    BOLD, RESET, WHITE, blank, error, header, hint, label_cyan, label_dim, success,
};

#[derive(Subcommand)]
pub enum ConfigAction {
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

pub fn run(action: ConfigAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        ConfigAction::List => list(),
        ConfigAction::Get { key: Some(key) } => get(&key),
        ConfigAction::Get { key: None } => list(),
        ConfigAction::Set { key, value } => set(&key, &value),
        ConfigAction::Unset { key } => unset(&key),
        ConfigAction::Edit => edit(),
        ConfigAction::Path => path(),
    }
}

fn list() -> Result<(), Box<dyn std::error::Error>> {
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

fn get(key: &str) -> Result<(), Box<dyn std::error::Error>> {
    if config::lookup_key(key).is_none() {
        unknown_key_error(key);
    }
    let cfg = config::load();
    println!("{}", config::get_value(&cfg, key));
    Ok(())
}

fn set(key: &str, values: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if config::lookup_key(key).is_none() {
        unknown_key_error(key);
    }

    match config::set_key(key, values) {
        Ok(display_val) => {
            success(&format!("{key} = {display_val}"));
            hint(&format!("  {}", config::config_path().display()));
        }
        Err(e) => {
            error(&e.to_string());
            std::process::exit(1);
        }
    }
    Ok(())
}

fn unset(key: &str) -> Result<(), Box<dyn std::error::Error>> {
    if config::lookup_key(key).is_none() {
        unknown_key_error(key);
    }

    match config::unset_key(key) {
        Ok(default_val) => {
            success(&format!("{key} reset to default ({default_val})"));
            hint(&format!("  {}", config::config_path().display()));
        }
        Err(e) => {
            error(&e.to_string());
            std::process::exit(1);
        }
    }
    Ok(())
}

fn edit() -> Result<(), Box<dyn std::error::Error>> {
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

fn path() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", config::config_path().display());
    Ok(())
}

fn unknown_key_error(key: &str) -> ! {
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
