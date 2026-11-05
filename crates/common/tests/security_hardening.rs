// SPDX-License-Identifier: AGPL-3.0-only

use common::{
    auth::oauth::{
        cimd::CimdClient, client::ResolvedClient, client_id::ClientMeta, scope::OAuthScopes,
    },
    network::forwarded::is_trusted_proxy,
};
use registry::{
    schema::{enums::OAuthScope, structs::OAuthClient},
    types::{ipmask::IpAddrOrMask, map::Map},
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
