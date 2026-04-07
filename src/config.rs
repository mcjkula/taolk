use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub wallet: Wallet,
    pub network: Network,
    pub security: Security,
    pub ui: Ui,
    pub notifications: Notifications,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Wallet {
    pub default: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Network {
    pub node: String,
    pub mirrors: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Security {
    pub lock_timeout: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Ui {
    pub sidebar_width: u16,
    pub mouse: bool,
    pub timestamp_format: String,
    pub date_format: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Notifications {
    pub enabled: bool,
    pub volume: u8,
    pub dm: bool,
    pub ambient: bool,
    pub mention: bool,
}

impl Default for Network {
    fn default() -> Self {
        Self {
            node: "wss://entrypoint-finney.opentensor.ai:443".into(),
            mirrors: Vec::new(),
        }
    }
}

impl Default for Security {
    fn default() -> Self {
        Self { lock_timeout: 300 }
    }
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            sidebar_width: 28,
            mouse: true,
            timestamp_format: "%H:%M".into(),
            date_format: "%Y-%m-%d %H:%M".into(),
        }
    }
}

impl Default for Notifications {
    fn default() -> Self {
        Self {
            enabled: true,
            volume: 70,
            dm: true,
            ambient: true,
            mention: true,
        }
    }
}

pub fn load() -> Config {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => toml::from_str(&content).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("taolk")
        .join("config.toml")
}

pub struct KeyDef {
    pub key: &'static str,
    pub section: &'static str,
    pub field: &'static str,
    pub description: &'static str,
    pub default_display: &'static str,
}

pub const KEYS: &[KeyDef] = &[
    KeyDef {
        key: "wallet.default",
        section: "wallet",
        field: "default",
        description: "Default wallet name",
        default_display: "\u{2014}",
    },
    KeyDef {
        key: "network.node",
        section: "network",
        field: "node",
        description: "Subtensor node WebSocket URL",
        default_display: "wss://entrypoint-finney.opentensor.ai:443",
    },
    KeyDef {
        key: "network.mirrors",
        section: "network",
        field: "mirrors",
        description: "SAMP mirror URLs",
        default_display: "\u{2014}",
    },
    KeyDef {
        key: "security.lock_timeout",
        section: "security",
        field: "lock_timeout",
        description: "Auto-lock timeout in seconds (0=off)",
        default_display: "300",
    },
    KeyDef {
        key: "ui.sidebar_width",
        section: "ui",
        field: "sidebar_width",
        description: "Sidebar width in columns",
        default_display: "28",
    },
    KeyDef {
        key: "ui.mouse",
        section: "ui",
        field: "mouse",
        description: "Enable mouse support",
        default_display: "true",
    },
    KeyDef {
        key: "ui.timestamp_format",
        section: "ui",
        field: "timestamp_format",
        description: "Message time format (chrono strftime)",
        default_display: "%H:%M",
    },
    KeyDef {
        key: "ui.date_format",
        section: "ui",
        field: "date_format",
        description: "Full date format (chrono strftime)",
        default_display: "%Y-%m-%d %H:%M",
    },
    KeyDef {
        key: "notifications.enabled",
        section: "notifications",
        field: "enabled",
        description: "Play notification sounds",
        default_display: "true",
    },
    KeyDef {
        key: "notifications.volume",
        section: "notifications",
        field: "volume",
        description: "Notification volume (0-100)",
        default_display: "70",
    },
    KeyDef {
        key: "notifications.dm",
        section: "notifications",
        field: "dm",
        description: "Sound for direct messages",
        default_display: "true",
    },
    KeyDef {
        key: "notifications.ambient",
        section: "notifications",
        field: "ambient",
        description: "Sound for channel/group activity",
        default_display: "true",
    },
    KeyDef {
        key: "notifications.mention",
        section: "notifications",
        field: "mention",
        description: "Sound when @-mentioned in a channel/group",
        default_display: "true",
    },
];

pub fn lookup_key(key: &str) -> Option<&'static KeyDef> {
    KEYS.iter().find(|k| k.key == key)
}

pub fn suggest_key(typo: &str) -> Option<&'static KeyDef> {
    KEYS.iter()
        .map(|k| (k, levenshtein(k.key, typo)))
        .filter(|(_, d)| *d <= 3)
        .min_by_key(|(_, d)| *d)
        .map(|(k, _)| k)
}

pub fn get_value(config: &Config, key: &str) -> String {
    match key {
        "wallet.default" => config
            .wallet
            .default
            .clone()
            .unwrap_or_else(|| "\u{2014}".into()),
        "network.node" => config.network.node.clone(),
        "network.mirrors" => {
            if config.network.mirrors.is_empty() {
                "\u{2014}".into() // em-dash
            } else {
                config.network.mirrors.join(", ")
            }
        }
        "security.lock_timeout" => config.security.lock_timeout.to_string(),
        "ui.sidebar_width" => config.ui.sidebar_width.to_string(),
        "ui.mouse" => config.ui.mouse.to_string(),
        "ui.timestamp_format" => config.ui.timestamp_format.clone(),
        "ui.date_format" => config.ui.date_format.clone(),
        "notifications.enabled" => config.notifications.enabled.to_string(),
        "notifications.volume" => config.notifications.volume.to_string(),
        "notifications.dm" => config.notifications.dm.to_string(),
        "notifications.ambient" => config.notifications.ambient.to_string(),
        "notifications.mention" => config.notifications.mention.to_string(),
        _ => String::new(),
    }
}

/// True only if the key is present in the on-disk TOML file (not a serde default).
pub fn is_user_set(key: &str) -> bool {
    let def = match lookup_key(key) {
        Some(d) => d,
        None => return false,
    };
    let content = match std::fs::read_to_string(config_path()) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let table: toml::Table = match content.parse() {
        Ok(t) => t,
        Err(_) => return false,
    };
    table
        .get(def.section)
        .and_then(|s| s.get(def.field))
        .is_some()
}

/// Set one key in the TOML file without touching defaults of unrelated keys.
pub fn set_key(key: &str, raw: &[String]) -> Result<String, String> {
    let def = lookup_key(key).ok_or_else(|| format!("Unknown key: {key}"))?;

    let toml_value = match key {
        "wallet.default" | "network.node" | "ui.timestamp_format" | "ui.date_format" => {
            toml::Value::String(raw.join(" "))
        }
        "network.mirrors" => {
            let items: Vec<toml::Value> = raw
                .iter()
                .filter(|s| !s.is_empty())
                .map(|s| toml::Value::String(s.clone()))
                .collect();
            toml::Value::Array(items)
        }
        "security.lock_timeout" => {
            let v: u64 = parse_val(raw, "a number")?;
            toml::Value::Integer(v as i64)
        }
        "ui.sidebar_width" => {
            let v: u16 = parse_val(raw, "a number (0-65535)")?;
            toml::Value::Integer(v as i64)
        }
        "ui.mouse" => toml::Value::Boolean(parse_bool(raw)?),
        "notifications.enabled"
        | "notifications.dm"
        | "notifications.ambient"
        | "notifications.mention" => toml::Value::Boolean(parse_bool(raw)?),
        "notifications.volume" => {
            let v: u8 = parse_val(raw, "a number (0-100)")?;
            if v > 100 {
                return Err(format!("Expected 0-100, got {v}"));
            }
            toml::Value::Integer(v as i64)
        }
        _ => return Err(format!("Unknown key: {key}")),
    };

    let path = config_path();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut table: toml::Table = content.parse().unwrap_or_default();

    let section = table
        .entry(def.section)
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    if let Some(sec) = section.as_table_mut() {
        sec.insert(def.field.to_string(), toml_value);
    }

    write_table(&path, &table)?;

    let cfg = load();
    Ok(get_value(&cfg, key))
}

pub fn save(config: &Config) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = toml::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

fn write_table(path: &std::path::Path, table: &toml::Table) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = toml::to_string_pretty(table).map_err(|e| e.to_string())?;
    std::fs::write(path, content).map_err(|e| e.to_string())
}

pub fn unset_key(key: &str) -> Result<String, String> {
    let def = lookup_key(key).ok_or("unknown key")?;

    let path = config_path();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut table: toml::Table = content.parse().unwrap_or_default();

    if let Some(section) = table.get_mut(def.section)
        && let Some(sec_table) = section.as_table_mut()
    {
        sec_table.remove(def.field);
        if sec_table.is_empty() {
            table.remove(def.section);
        }
    }

    write_table(&path, &table)?;
    Ok(def.default_display.to_string())
}

fn parse_val<T: std::str::FromStr>(raw: &[String], expected: &str) -> Result<T, String> {
    let s = raw.first().map(|s| s.as_str()).unwrap_or("");
    s.parse::<T>()
        .map_err(|_| format!("Expected {expected}, got '{s}'"))
}

fn parse_bool(raw: &[String]) -> Result<bool, String> {
    let s = raw.first().map(|s| s.as_str()).unwrap_or("");
    match s {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("Expected 'true' or 'false', got '{s}'")),
    }
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a = a.as_bytes();
    let b = b.as_bytes();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        curr[0] = i;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}
