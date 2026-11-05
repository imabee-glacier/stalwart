// SPDX-License-Identifier: AGPL-3.0-only
use registry::{
    jmap::IntoValue,
    pickle::{Pickle, PickledStream},
    schema::{
        enums::{NetworkListenerProtocol, Permission, ServiceProtocol},
        structs::{NetworkListener, Service, SystemSettings},
    },
    types::{EnumImpl, ObjectImpl, map::Map},
};

#[test]
fn pop_cannot_be_requested_but_old_storage_ids_remain_reserved() {
    assert!(NetworkListenerProtocol::parse("pop3").is_none());
    assert!(ServiceProtocol::parse("pop3").is_none());
    assert!(Permission::parse("pop3Authenticate").is_none());
    assert!(NetworkListenerProtocol::from_id(4).unwrap().is_retired());
    assert!(ServiceProtocol::from_id(2).unwrap().is_retired());
    assert!(Permission::from_id(161).unwrap().is_retired());
    assert_eq!(NetworkListenerProtocol::ManageSieve.to_id(), 5);
    assert_eq!(ServiceProtocol::Smtp.to_id(), 3);
    assert!(Permission::ALL.iter().all(|p| !p.is_retired()));
}

#[test]
fn old_listeners_decode_but_cannot_be_saved_as_active_configuration() {
    let listener = NetworkListener {
        protocol: NetworkListenerProtocol::RetiredPop3,
        ..Default::default()
    };
    let bytes = listener.to_pickled_vec();
    let restored = NetworkListener::unpickle(&mut PickledStream::new(&bytes).unwrap()).unwrap();
    assert_eq!(restored, listener);
    let mut errors = vec![];
    assert!(!restored.validate(&mut errors));
}

#[test]
fn legacy_service_and_permission_entries_do_not_reappear_in_public_settings() {
    let mut settings = SystemSettings::default();
    settings
        .services
        .set(ServiceProtocol::RetiredPop3, Service::default());
    let bytes = settings.to_pickled_vec();
    let restored = SystemSettings::unpickle(&mut PickledStream::new(&bytes).unwrap()).unwrap();
    let public = serde_json::to_string(&restored.into_value()).unwrap();
    assert!(!public.to_lowercase().contains("pop3"));
    let permissions = Map::new(vec![Permission::RetiredPop3Authenticate]);
    assert_eq!(
        serde_json::to_value(permissions.into_value()).unwrap(),
        serde_json::json!({})
    );
}

#[test]
fn pop_telemetry_is_gone_and_reserved_ids_stay_unused() {
    for name in ["pop3.connection-start", "pop3.raw-input", "pop3.error"] {
        assert!(trc::EventType::parse(name).is_none());
    }
    for id in 330..=347 {
        assert!(trc::EventType::from_id(id).is_none(), "event id {id}");
    }
    for id in [14, 19, 194, 195] {
        assert!(trc::MetricType::from_id(id).is_none(), "metric id {id}");
    }
    assert!(trc::MetricType::parse("pop3.active-connections").is_none());
    for event in trc::EventType::variants() {
        let text = format!(
            "{} {} {} {}",
            event.as_str(),
            event.description(),
            event.otel_metric_name(),
            event.prom_metric_name()
        );
        assert!(!text.to_lowercase().contains("pop3"), "{text}");
    }
    for metric in trc::MetricType::variants() {
        let text = format!(
            "{} {} {} {}",
            metric.as_str(),
            metric.description(),
            metric.otel_name(),
            metric.prom_name()
        );
        assert!(!text.to_lowercase().contains("pop3"), "{text}");
    }
}

#[test]
fn stored_event_filters_drop_removed_ids_without_losing_the_rest() {
    let kept = trc::EventType::Imap(trc::ImapEvent::ConnectionStart);
    // Version marker, then the map as stored inside a tracer's event filter.
    let mut bytes = vec![0u8];
    3u64.pickle(&mut bytes);
    kept.pickle(&mut bytes);
    332u16.pickle(&mut bytes); // former pop3.connection-start
    trc::EventType::Imap(trc::ImapEvent::ConnectionEnd).pickle(&mut bytes);
    let restored = Map::<trc::EventType>::unpickle(&mut PickledStream::new(&bytes).unwrap())
        .expect("filter with a removed event must still decode");
    assert_eq!(
        restored,
        Map::new(vec![
            kept,
            trc::EventType::Imap(trc::ImapEvent::ConnectionEnd)
        ])
    );

    // Truncated data is still rejected rather than silently accepted.
    assert!(
        Map::<trc::EventType>::unpickle(&mut PickledStream::new(&bytes[..3]).unwrap()).is_none()
    );
}
