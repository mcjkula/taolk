mod common;

use common::{charlie_session as make_session, dave_pubkey as other_pubkey};
use taolk::types::BlockRef;

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
