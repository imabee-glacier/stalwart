// SPDX-License-Identifier: AGPL-3.0-only

use common::{
    auth::oauth::{
        cimd::CimdClient,
        client::ResolvedClient,
        client_id::ClientMeta,
        grant::{ROTATION_RETRY_WINDOW_SECS, StoredOAuthGrant},
        scope::OAuthScopes,
    },
    network::forwarded::is_trusted_proxy,
};
use registry::{
    pickle::{Pickle, PickledStream},
    schema::{
        enums::{OAuthClientKind, OAuthScope},
        structs::{OAuthClient, OAuthGrant},
    },
    types::{datetime::UTCDateTime, ipmask::IpAddrOrMask, map::Map},
};
use std::net::IpAddr;
use types::id::Id;

fn ip(value: &str) -> IpAddr {
    value.parse().unwrap()
}

fn network(value: &str) -> IpAddrOrMask {
    value.parse().unwrap()
}

#[test]
fn forwarded_headers_trust_only_private_peers_by_default() {
    for peer in ["127.0.0.1", "::1", "10.1.2.3", "192.168.0.10", "172.16.5.4"] {
        assert!(is_trusted_proxy(ip(peer), &[]), "{peer} should be trusted");
    }
    for peer in ["8.8.8.8", "203.0.114.7", "2606:4700::1111"] {
        assert!(
            !is_trusted_proxy(ip(peer), &[]),
            "{peer} must not be trusted"
        );
    }
}

#[test]
fn configured_trusted_proxies_are_authoritative() {
    let proxies = [network("203.0.113.0/24"), network("2001:db8::1")];
    assert!(is_trusted_proxy(ip("203.0.113.9"), &proxies));
    assert!(is_trusted_proxy(ip("2001:db8::1"), &proxies));
    assert!(!is_trusted_proxy(ip("203.0.114.9"), &proxies));
    // Once a list is configured, private peers are no longer implicitly trusted.
    assert!(!is_trusted_proxy(ip("127.0.0.1"), &proxies));
    assert!(!is_trusted_proxy(ip("10.0.0.1"), &proxies));
}

fn registered(member_tenant_id: Option<Id>) -> ResolvedClient {
    ResolvedClient::Registered(Box::new(OAuthClient {
        client_id: "client".into(),
        member_tenant_id,
        scopes: Map::new(vec![
            OAuthScope::Mail,
            OAuthScope::Security,
            OAuthScope::Settings,
            OAuthScope::Full,
        ]),
        ..Default::default()
    }))
}

#[test]
fn unverified_clients_cannot_obtain_account_scopes() {
    let unverified = [
        ResolvedClient::Stateless(ClientMeta {
            redirect_uris: vec!["https://app.example/cb".into()],
            scopes: OAuthScopes::ALL,
            client_name: None,
        }),
        ResolvedClient::Cimd(Some(Box::new(CimdClient {
            client_host: "app.example".into(),
            client_name: None,
            redirect_uris: vec!["https://app.example/cb".into()],
            post_logout_redirect_uris: vec![],
            scopes: OAuthScopes::ALL,
        }))),
        registered(Some(Id::new(7))),
    ];
    for client in unverified {
        assert!(!client.is_verified());
        let allowed = client.allowed_scopes().unwrap();
        for scope in [
            OAuthScopes::SECURITY,
            OAuthScopes::SETTINGS,
            OAuthScopes::FULL,
        ] {
            assert!(!allowed.contains(scope), "{client:?} kept {scope:?}");
        }
        assert!(allowed.contains(OAuthScopes::MAIL));
    }
}

#[test]
fn administrator_registered_clients_keep_account_scopes() {
    let client = registered(None);
    assert!(client.is_verified());
    assert!(
        client
            .allowed_scopes()
            .unwrap()
            .contains(OAuthScopes::STALWART)
    );
}

fn first_party(member_tenant_id: Option<Id>) -> ResolvedClient {
    ResolvedClient::Registered(Box::new(OAuthClient {
        client_id: "webmail".into(),
        member_tenant_id,
        first_party: true,
        scopes: Map::new(vec![OAuthScope::Mail, OAuthScope::Contacts]),
        ..Default::default()
    }))
}

#[test]
fn first_party_clients_skip_consent_and_nothing_else() {
    let client = first_party(None);
    assert!(!client.requires_consent());
    // Still an ordinary registered client: the kind recorded on its grants is
    // not FirstParty, which would add FULL to every token it is issued.
    assert_eq!(client.kind(), OAuthClientKind::Registered);
    let allowed = client.allowed_scopes().unwrap();
    assert!(allowed.contains(OAuthScopes::MAIL));
    for scope in [
        OAuthScopes::FULL,
        OAuthScopes::SECURITY,
        OAuthScopes::SETTINGS,
    ] {
        assert!(!allowed.contains(scope), "first-party gained {scope:?}");
    }
    assert_eq!(
        client.granted_scopes(OAuthScopes::MAIL),
        OAuthScopes::MAIL,
        "first-party must not widen what is granted"
    );
}

#[test]
fn tenant_clients_cannot_be_first_party() {
    assert!(first_party(Some(Id::new(7))).requires_consent());
    assert!(registered(None).requires_consent());
}

fn pickled(client: &OAuthClient) -> Vec<u8> {
    let mut out = vec![0u8];
    client.pickle(&mut out);
    out
}

fn unpickled(bytes: &[u8]) -> OAuthClient {
    OAuthClient::unpickle(&mut PickledStream::new(bytes).unwrap()).unwrap()
}

#[test]
fn first_party_survives_storage() {
    let client = OAuthClient {
        client_id: "webmail".into(),
        first_party: true,
        ..Default::default()
    };
    assert_eq!(unpickled(&pickled(&client)), client);
}

#[test]
fn clients_stored_before_first_party_existed_read_as_not_first_party() {
    // Rewrite a current record into the previous layout: thirteen fields, no
    // trailing flag. Small block header: u16 LE (len << 2 | 0b10), then
    // (fields << 1 | 1), (flags << 1 | 1).
    let client = OAuthClient {
        client_id: "legacy".into(),
        redirect_uris: Map::new(vec!["https://mail.example/cb".into()]),
        ..Default::default()
    };
    let mut bytes = pickled(&client);
    assert_eq!(bytes[3], (14 << 1) | 1);
    assert_eq!(bytes.pop(), Some(0));
    bytes[3] = (13 << 1) | 1;
    let len = bytes.len() - 1 - 2;
    bytes[1..3].copy_from_slice(&(((len << 2) | 0b10) as u16).to_le_bytes());

    let read = unpickled(&bytes);
    assert!(!read.first_party);
    assert_eq!(read, client);
}

fn pending_grant(generation: u64, rotated_at: i64) -> StoredOAuthGrant {
    StoredOAuthGrant {
        id: 1,
        revision: 0,
        grant: Box::new(OAuthGrant {
            generation,
            pending: true,
            last_used_at: UTCDateTime::from_timestamp(rotated_at),
            ..Default::default()
        }),
    }
}

#[test]
fn superseded_refresh_token_is_honoured_only_briefly() {
    let rotated_at = 1_800_000_000;
    let grant = pending_grant(3, rotated_at);
    let at = |offset: u64| rotated_at as u64 + offset;

    // The response carrying generation 4 was lost: retrying with 3 works, briefly.
    assert!(grant.is_current_at(3, at(0)));
    assert!(grant.is_current_at(3, at(ROTATION_RETRY_WINDOW_SECS)));
    // Later, 3 is a replay. Whoever presents it is not the client that rotated.
    assert!(!grant.is_current_at(3, at(ROTATION_RETRY_WINDOW_SECS + 1)));
    assert!(!grant.is_current_at(3, at(86_400)));
    // The token actually issued keeps working however late it is first used.
    assert!(grant.is_current_at(4, at(86_400)));
    // Older generations never were current.
    assert!(!grant.is_current_at(2, at(0)));
}
