use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum DbAction {
    Clear {
        #[arg(long)]
        wallet: Option<String>,
    },
}

pub fn run(action: DbAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        DbAction::Clear { wallet } => clear(wallet),
    }
}

fn clear(wallet_filter: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let base = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("taolk");

    if !base.exists() {
        println!("No data directory found at {}", base.display());
        return Ok(());
    }

    let wallets: Vec<String> = match wallet_filter {
        Some(name) => {
            let dir = base.join(&name);
            if !dir.exists() {
                println!("No data for wallet \"{name}\"");
                return Ok(());
            }
            vec![name]
        }
        None => std::fs::read_dir(&base)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect(),
    };

    if wallets.is_empty() {
        println!("No wallet data found");
        return Ok(());
    }

    for wallet in &wallets {
        let wallet_dir = base.join(wallet);
        let mut cleared = 0usize;
        for entry in std::fs::read_dir(&wallet_dir)?.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                let db_path = path.join("messages.db");
                if db_path.exists() {
                    std::fs::remove_file(&db_path)?;
                    cleared += 1;
                }
            }
        }
        if cleared > 0 {
            println!("Cleared {cleared} database(s) for wallet \"{wallet}\"");
        }
    }
    Ok(())
}
