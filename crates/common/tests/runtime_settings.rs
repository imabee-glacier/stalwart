// SPDX-License-Identifier: AGPL-3.0-only

use common::config::runtime::RuntimeConfig;
use registry::{
    schema::structs::{Branding, DataRetention},
    types::duration::Duration,
};
use store::{
    RegistryStore,
    registry::{bootstrap::Bootstrap, write::RegistryWrite},
};

#[tokio::test]
async fn configured_features_load_and_reload_without_activation() {
    let local = std::env::temp_dir().join(format!(
        "stalwart-runtime-test-{}-missing.json",
        std::process::id()
    ));
    let registry = RegistryStore::init(local, false).await.unwrap();
    let mut bp = Bootstrap::new(registry).await;
    let mut retention = DataRetention {
        archive_deleted_items_for: Some(Duration::from(std::time::Duration::from_secs(86400))),
        archive_deleted_accounts_for: Some(Duration::from(std::time::Duration::from_secs(172800))),
        hold_traces_for: Some(Duration::from(std::time::Duration::from_secs(259200))),
        hold_metrics_for: Some(Duration::from(std::time::Duration::from_secs(345600))),
        ..Default::default()
    };
    bp.registry
        .write(RegistryWrite::insert(&retention.clone().into()))
        .await
        .unwrap();
    bp.registry
        .write(RegistryWrite::insert(
            &Branding {
                logo: Some("https://example.org/logo.png".into()),
            }
            .into(),
        ))
        .await
        .unwrap();
    let first = RuntimeConfig::parse(&mut bp).await;
    assert_eq!(first.logo.as_deref(), Some("https://example.org/logo.png"));
    assert_eq!(first.deleted_items_retention.unwrap().as_secs(), 86400);
    assert_eq!(first.deleted_accounts_retention.unwrap().as_secs(), 172800);
    assert_eq!(first.trace_retention.unwrap().as_secs(), 259200);
    assert_eq!(first.metrics_retention.unwrap().as_secs(), 345600);
    assert!(first.metrics_interval.time_to_next().as_secs() > 0);

    retention.archive_deleted_items_for =
        Some(Duration::from(std::time::Duration::from_secs(604800)));
    bp.registry
        .write(RegistryWrite::insert(&retention.into()))
        .await
        .unwrap();
    let reloaded = RuntimeConfig::parse(&mut bp).await;
    assert_eq!(reloaded.deleted_items_retention.unwrap().as_secs(), 604800);
    assert_eq!(reloaded.logo, first.logo);
    assert!(bp.errors.is_empty(), "{:?}", bp.errors);
}
