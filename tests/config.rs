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
    assert_eq!(config::KEYS.len(), 14);
}

// --- set_key validation errors (no filesystem write needed) ---

#[test]
fn set_key_unknown_key() {
    let result = config::set_key("nonexistent.key", &["whatever".into()]);
    assert!(result.is_err());
}

#[test]
fn set_key_sidebar_width_invalid_value() {
    let result = config::set_key("ui.sidebar_width", &["abc".into()]);
    assert!(result.is_err());
}

#[test]
fn set_key_mouse_invalid_value() {
    let result = config::set_key("ui.mouse", &["maybe".into()]);
    assert!(result.is_err());
}

#[test]
fn set_key_lock_timeout_invalid_value() {
    let result = config::set_key("security.lock_timeout", &["not_a_number".into()]);
    assert!(result.is_err());
}

#[test]
fn set_key_volume_out_of_range() {
    let result = config::set_key("notifications.volume", &["200".into()]);
    assert!(result.is_err());
}

#[test]
fn set_key_bool_fields_reject_non_bool() {
    for key in [
        "security.require_password_per_send",
        "notifications.enabled",
        "notifications.dm",
        "notifications.ambient",
        "notifications.mention",
    ] {
        let result = config::set_key(key, &["yes".into()]);
        assert!(result.is_err(), "expected error for {key}");
    }
}

// --- unset_key unknown key ---

#[test]
fn unset_key_unknown() {
    let result = config::unset_key("nonexistent.key");
    assert!(result.is_err());
}

// --- get_value exhaustive ---

#[test]
fn get_value_all_defaults() {
    let cfg = Config::default();
    assert_eq!(config::get_value(&cfg, "ui.sidebar_width"), "28");
    assert_eq!(config::get_value(&cfg, "ui.timestamp_format"), "%H:%M");
    assert_eq!(config::get_value(&cfg, "ui.date_format"), "%Y-%m-%d %H:%M");
    assert_eq!(
        config::get_value(&cfg, "security.require_password_per_send"),
        "false"
    );
    assert_eq!(config::get_value(&cfg, "notifications.enabled"), "true");
    assert_eq!(config::get_value(&cfg, "notifications.volume"), "70");
    assert_eq!(config::get_value(&cfg, "notifications.dm"), "true");
    assert_eq!(config::get_value(&cfg, "notifications.ambient"), "true");
    assert_eq!(config::get_value(&cfg, "notifications.mention"), "true");
}

#[test]
fn get_value_unknown_key_returns_empty() {
    let cfg = Config::default();
    assert_eq!(config::get_value(&cfg, "nonexistent"), "");
}

#[test]
fn get_value_wallet_default_set() {
    let mut cfg = Config::default();
    cfg.wallet.default = Some("my-wallet".into());
    assert_eq!(config::get_value(&cfg, "wallet.default"), "my-wallet");
}

// --- suggest_key ---

#[test]
fn suggest_key_close_typo() {
    assert_eq!(
        config::suggest_key("ui.sidbar_width").unwrap().key,
        "ui.sidebar_width"
    );
}

#[test]
fn suggest_key_exact_match() {
    assert_eq!(config::suggest_key("ui.mouse").unwrap().key, "ui.mouse");
}

// --- KeyDef coverage ---

#[test]
fn key_defs_have_non_empty_fields() {
    for kd in config::KEYS {
        assert!(!kd.key.is_empty());
        assert!(!kd.section.is_empty());
        assert!(!kd.field.is_empty());
        assert!(!kd.description.is_empty());
        assert!(!kd.default_display.is_empty());
        assert!(
            kd.key.contains('.'),
            "key should be section.field: {}",
            kd.key
        );
    }
}

// --- set_key + unset_key filesystem round-trip (single test to avoid parallelism issues) ---

#[test]
fn set_key_and_unset_key_filesystem_round_trip() {
    let path = config::config_path();
    let existed = path.exists();
    let original = if existed {
        std::fs::read_to_string(&path).ok()
    } else {
        None
    };

    // String fields
    if config::set_key("wallet.default", &["my-wallet".into()]).is_err() {
        restore(&path, &original);
        return; // read-only FS
    }
    assert_eq!(
        config::get_value(&config::load(), "wallet.default"),
        "my-wallet"
    );

    assert!(config::set_key("network.node", &["wss://custom.example.com".into()]).is_ok());
    assert!(config::set_key("ui.timestamp_format", &["%H:%M:%S".into()]).is_ok());
    assert!(config::set_key("ui.date_format", &["%d/%m/%Y".into()]).is_ok());

    // Array field
    assert!(
        config::set_key(
            "network.mirrors",
            &[
                "https://a.example.com".into(),
                "https://b.example.com".into()
            ]
        )
        .is_ok()
    );
    let mirrors = config::get_value(&config::load(), "network.mirrors");
    assert!(mirrors.contains("https://a.example.com"));

    // Numeric fields
    assert_eq!(
        config::set_key("security.lock_timeout", &["600".into()]).unwrap(),
        "600"
    );
    assert_eq!(
        config::set_key("ui.sidebar_width", &["40".into()]).unwrap(),
        "40"
    );
    assert_eq!(
        config::set_key("notifications.volume", &["80".into()]).unwrap(),
        "80"
    );

    // Bool fields
    for (key, val, expected) in [
        ("ui.mouse", "false", "false"),
        ("security.require_password_per_send", "true", "true"),
        ("notifications.enabled", "false", "false"),
        ("notifications.dm", "true", "true"),
        ("notifications.ambient", "false", "false"),
        ("notifications.mention", "true", "true"),
    ] {
        assert_eq!(
            config::set_key(key, &[val.into()]).unwrap(),
            expected,
            "key={key}"
        );
    }

    // is_user_set
    assert!(config::is_user_set("ui.mouse"));

    // unset_key
    let default_val = config::unset_key("ui.sidebar_width").unwrap();
    assert_eq!(default_val, "28");

    // save/load round-trip
    let mut cfg = Config::default();
    cfg.ui.sidebar_width = 42;
    cfg.security.lock_timeout = 999;
    cfg.wallet.default = Some("roundtrip-test".into());
    config::save(&cfg).unwrap();
    let loaded = config::load();
    assert_eq!(loaded.ui.sidebar_width, 42);
    assert_eq!(loaded.security.lock_timeout, 999);
    assert_eq!(loaded.wallet.default.as_deref(), Some("roundtrip-test"));

    restore(&path, &original);
}

fn restore(path: &std::path::Path, original: &Option<String>) {
    match original {
        Some(c) => {
            std::fs::write(path, c).ok();
        }
        None => {
            std::fs::remove_file(path).ok();
        }
    }
}

#[test]
fn is_user_set_returns_false_for_unknown() {
    assert!(!config::is_user_set("nonexistent.key"));
}

// --- levenshtein ---

#[test]
fn suggest_key_various_typos() {
    assert_eq!(config::suggest_key("ui.moues").unwrap().key, "ui.mouse");
    assert_eq!(
        config::suggest_key("network.nod").unwrap().key,
        "network.node"
    );
    assert_eq!(
        config::suggest_key("notifications.volum").unwrap().key,
        "notifications.volume"
    );
}

// save/reload round-trip is tested inside set_key_and_unset_key_filesystem_round_trip
