// SPDX-License-Identifier: AGPL-3.0-only

use registry::{
    pickle::{Pickle, PickledStream, begin_block, end_block},
    schema::{enums::ClientIpSource, structs::Http},
    types::{ObjectImpl, ipmask::IpAddrOrMask, map::Map},
};

/// Writes an `Http` record exactly as releases before `trustedProxies` did.
fn legacy_http_record(http: &Http) -> Vec<u8> {
    let mut out = vec![Http::VERSION];
    let mut bitmap = 0u64;
    for (bit, set) in [
        http.rate_limit_authenticated.is_some(),
        http.rate_limit_anonymous.is_some(),
        http.enable_hsts,
        http.use_permissive_cors,
        http.redirect_root.is_some(),
    ]
    .into_iter()
    .enumerate()
    {
        if set {
            bitmap |= 1 << bit;
        }
    }
    let block = begin_block(&mut out, 9, 5, bitmap);
    if let Some(value) = &http.rate_limit_authenticated {
        value.pickle(&mut out);
    }
    if let Some(value) = &http.rate_limit_anonymous {
        value.pickle(&mut out);
    }
    http.allowed_endpoints.pickle(&mut out);
    http.response_headers.pickle(&mut out);
    http.client_ip_source.pickle(&mut out);
    http.forwarded_hops.pickle(&mut out);
    if let Some(value) = &http.redirect_root {
        value.pickle(&mut out);
    }
    end_block(&mut out, block);
    out
}

fn network(value: &str) -> IpAddrOrMask {
    value.parse().expect("valid network")
}

#[test]
fn existing_http_settings_load_without_trusted_proxies() {
    let http = Http {
        client_ip_source: ClientIpSource::XForwardedFor,
        forwarded_hops: 2,
        enable_hsts: true,
        ..Default::default()
    };
    let bytes = legacy_http_record(&http);
    let restored = Http::unpickle(&mut PickledStream::new(&bytes).unwrap()).unwrap();
    assert_eq!(restored, http);
    assert!(restored.trusted_proxies.is_empty());
}

#[test]
fn trusted_proxies_round_trip_and_validate() {
    let http = Http {
        client_ip_source: ClientIpSource::Forwarded,
        trusted_proxies: Map::new(vec![network("203.0.113.0/24"), network("2001:db8::1")]),
        ..Default::default()
    };
    let bytes = http.to_pickled_vec();
    let restored = Http::unpickle(&mut PickledStream::new(&bytes).unwrap()).unwrap();
    assert_eq!(restored, http);
    let mut errors = Vec::new();
    assert!(restored.validate(&mut errors), "{errors:?}");
}
