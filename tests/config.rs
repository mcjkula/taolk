use taolk::config::{self, Config};

#[test]
fn default_config_values() {
    let cfg = Config::default();
    assert_eq!(
        cfg.network.node,
        "wss://entrypoint-finney.opentensor.ai:443"
    );
    assert_eq!(cfg.security.lock_timeout, 300);
    assert_eq!(cfg.ui.sidebar_width, 28);
    assert!(cfg.ui.mouse);
}

#[test]
fn get_value_node() {
    let cfg = Config::default();
    assert_eq!(
        config::get_value(&cfg, "network.node"),
        "wss://entrypoint-finney.opentensor.ai:443"
    );
}

#[test]
fn get_value_empty_mirrors() {
    let cfg = Config::default();
    assert_eq!(config::get_value(&cfg, "network.mirrors"), "\u{2014}");
}

#[test]
fn get_value_populated_mirrors() {
    let mut cfg = Config::default();
    cfg.network.mirrors = vec!["url1".into(), "url2".into()];
    assert_eq!(config::get_value(&cfg, "network.mirrors"), "url1, url2");
}

#[test]
fn get_value_lock_timeout() {
    let cfg = Config::default();
    assert_eq!(config::get_value(&cfg, "security.lock_timeout"), "300");
}

#[test]
fn get_value_mouse() {
    let cfg = Config::default();
    assert_eq!(config::get_value(&cfg, "ui.mouse"), "true");
}

#[test]
fn get_value_wallet_default_none() {
    let cfg = Config::default();
    assert!(cfg.wallet.default.is_none());
    assert_eq!(config::get_value(&cfg, "wallet.default"), "\u{2014}");
}

#[test]
fn lookup_key_all_known() {
    for kd in config::KEYS {
        assert!(
            config::lookup_key(kd.key).is_some(),
            "lookup_key failed for '{}'",
            kd.key
        );
    }
}

#[test]
fn lookup_key_unknown() {
    assert!(config::lookup_key("nonexistent.key").is_none());
}

#[test]
fn suggest_key_typo() {
    let suggestion = config::suggest_key("ui.mous");
    assert!(suggestion.is_some());
    assert_eq!(suggestion.unwrap().key, "ui.mouse");
}

#[test]
fn suggest_key_no_match() {
    assert!(config::suggest_key("zzzzzzz").is_none());
}

#[test]
fn key_count() {
    assert_eq!(config::KEYS.len(), 17);
}
