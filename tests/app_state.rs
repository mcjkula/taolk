use schnorrkel::keys::{ExpansionMode, MiniSecretKey};
use taolk::db::Db;
use taolk::extrinsic::ChainInfo;
use taolk::metadata::AccountInfoLayout;
use taolk::session::Session;
use taolk::types::{BlockRef, Pubkey};
use zeroize::Zeroizing;

const SEED: [u8; 32] = [0xCC; 32];

fn keypair_from_seed(seed: &[u8; 32]) -> schnorrkel::Keypair {
    MiniSecretKey::from_bytes(seed)
        .unwrap()
        .expand_to_keypair(ExpansionMode::Ed25519)
}

fn ci() -> ChainInfo {
    ChainInfo {
        genesis_hash: [0; 32],
        spec_version: 1,
        tx_version: 1,
        account_info_layout: AccountInfoLayout {
            free_offset: 16,
            free_width: 8,
        },
        errors: Default::default(),
        chain_name: "test".into(),
    }
}

fn make_session() -> Session {
    let db = Db::open_in_memory(&SEED).unwrap();
    Session::new(
        keypair_from_seed(&SEED),
        Zeroizing::new(SEED),
        "ws://test".into(),
        ci(),
        db,
    )
}

fn other_pubkey() -> Pubkey {
    Pubkey(keypair_from_seed(&[0xDD; 32]).public.to_bytes())
}

// ---------------------------------------------------------------------------
// known_contacts after creating a thread
// ---------------------------------------------------------------------------

#[test]
fn session_known_contacts_after_thread() {
    let mut session = make_session();
    let peer = other_pubkey();
    session.create_thread(peer).unwrap();

    let contacts = session.known_contacts();
    assert!(
        contacts.iter().any(|(_, pk)| *pk == peer),
        "peer should appear in known_contacts after create_thread"
    );
}

// ---------------------------------------------------------------------------
// cleanup_pending removes a pending thread
// ---------------------------------------------------------------------------

#[test]
fn session_cleanup_pending_thread() {
    let mut session = make_session();
    let peer = other_pubkey();
    session.create_thread(peer).unwrap();
    assert_eq!(session.threads.len(), 1);
    assert_eq!(session.threads[0].thread_ref, BlockRef::ZERO);

    let result = session.cleanup_pending();
    assert!(result.is_some());
    assert!(result.unwrap().removed_thread.is_some());
    assert_eq!(session.threads.len(), 0);
}

// ---------------------------------------------------------------------------
// cleanup_pending removes a pending channel
// ---------------------------------------------------------------------------

#[test]
fn session_cleanup_pending_channel() {
    let mut session = make_session();
    session.create_pending_channel("test-chan".into(), "creator".into());
    assert_eq!(session.channels.len(), 1);
    assert_eq!(session.channels[0].channel_ref, BlockRef::ZERO);

    let result = session.cleanup_pending();
    assert!(result.is_some());
    assert!(result.unwrap().removed_channel.is_some());
    assert_eq!(session.channels.len(), 0);
}

// ---------------------------------------------------------------------------
// cleanup_pending removes a pending group
// ---------------------------------------------------------------------------

#[test]
fn session_cleanup_pending_group() {
    let mut session = make_session();
    let peer = other_pubkey();
    session.create_pending_group(session.pubkey(), vec![session.pubkey(), peer]);
    assert_eq!(session.groups.len(), 1);
    assert_eq!(session.groups[0].group_ref, BlockRef::ZERO);

    let result = session.cleanup_pending();
    assert!(result.is_some());
    assert!(result.unwrap().removed_group.is_some());
    assert_eq!(session.groups.len(), 0);
}

// ---------------------------------------------------------------------------
// cleanup_pending returns None when nothing is pending
// ---------------------------------------------------------------------------

#[test]
fn session_cleanup_nothing_pending() {
    let mut session = make_session();
    let result = session.cleanup_pending();
    assert!(
        result.is_none(),
        "cleanup_pending should return None when nothing is pending"
    );
}
