/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 */

use registry::schema::structs::ZenohCoordinator;
use std::sync::Arc;

use crate::Coordinator;
pub mod pubsub;

#[derive(Debug)]
pub struct ZenohPubSub {
    session: zenoh::Session,
}

impl ZenohPubSub {
    pub async fn open(config: ZenohCoordinator) -> Result<Coordinator, String> {
        let zenoh_config = zenoh::Config::from_json5(&config.config)
            .map_err(|err| format!("Invalid Zenoh config: {}", err))?;
        validate_security(&zenoh_config)?;
        zenoh::open(zenoh_config)
            .await
            .map_err(|err| format!("Failed to create Zenoh session: {}", err))
            .map(|session| ZenohPubSub { session })
            .map(|store| Coordinator::Zenoh(Arc::new(store)))
    }
}

// The legacy RSA challenge exchange uses the unpatched Marvin-vulnerable
// private-key implementation. Reject its configuration rather than silently
// opening an unauthenticated connection after removing the Cargo feature.
fn validate_security(config: &zenoh::Config) -> Result<(), String> {
    for path in [
        "transport/auth/pubkey/public_key_pem",
        "transport/auth/pubkey/private_key_pem",
        "transport/auth/pubkey/public_key_file",
        "transport/auth/pubkey/private_key_file",
        "transport/auth/pubkey/known_keys_file",
        "transport/auth/pubkey/key_size",
        "transport/auth/usrpwd/user",
        "transport/auth/usrpwd/password",
        "transport/auth/usrpwd/dictionary_file",
    ] {
        let value = config
            .get_json(path)
            .map_err(|_| "Could not validate Zenoh authentication settings".to_string())?;
        if value.trim() != "null" {
            return Err("Legacy Zenoh RSA and username/password authentication are unavailable in this build. Use authenticated TLS transport.".into());
        }
    }
    let links = config
        .get_json("transport/unicast/max_links")
        .map_err(|_| "Could not validate Zenoh link settings".to_string())?
        .parse::<usize>()
        .map_err(|_| "Invalid Zenoh link limit".to_string())?;
    if links > 1 {
        return Err("Zenoh multilink is disabled because its legacy RSA exchange is affected by RUSTSEC-2023-0071. Use a single authenticated TLS link.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_authentication_fails_closed() {
        assert!(validate_security(&zenoh::Config::default()).is_ok());
        for value in [
            r#"{transport:{auth:{pubkey:{public_key_file:"key.pem"}}}}"#,
            r#"{transport:{unicast:{max_links:2}}}"#,
            r#"{transport:{auth:{usrpwd:{user:"alice",password:"secret"}}}}"#,
        ] {
            let config = zenoh::Config::from_json5(value).unwrap();
            assert!(validate_security(&config).is_err());
        }
    }
}
