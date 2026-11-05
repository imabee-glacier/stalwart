/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 */

pub mod alerts;
pub mod metrics;
pub mod tracers;
pub mod webhooks;

use tracers::log::spawn_log_tracer;
use tracers::otel::spawn_otel_tracer;
use tracers::stdout::spawn_console_tracer;
use trc::{Collector, ipc::subscriber::SubscriberBuilder};
use webhooks::spawn_webhook_tracer;

use crate::config::telemetry::{Telemetry, TelemetrySubscriberType};

impl Telemetry {
    pub fn enable(self) {
        // Spawn tracers
        for tracer in self.tracers.subscribers {
            tracer.typ.spawn(
                SubscriberBuilder::new(tracer.id)
                    .with_interests(tracer.interests)
                    .with_lossy(tracer.lossy),
            );
        }

        // Update global collector
        Collector::set_interests(self.tracers.interests);
        Collector::update_custom_levels(self.tracers.levels);
        Collector::set_metrics(self.metrics);
        Collector::reload();
    }

    pub fn update(self) {
        // Remove tracers that are no longer active
        let active_subscribers = Collector::get_subscribers();
        for subscribed_id in &active_subscribers {
            if !self
                .tracers
                .subscribers
                .iter()
                .any(|tracer| tracer.id == *subscribed_id)
            {
                Collector::remove_subscriber(subscribed_id.clone());
            }
        }

        // Activate new tracers or update existing ones
        for tracer in self.tracers.subscribers {
            if active_subscribers.contains(&tracer.id) {
                Collector::update_subscriber(tracer.id, tracer.interests, tracer.lossy);
            } else {
                tracer.typ.spawn(
                    SubscriberBuilder::new(tracer.id)
                        .with_interests(tracer.interests)
                        .with_lossy(tracer.lossy),
                );
            }
        }

        // Update global collector
        Collector::set_interests(self.tracers.interests);
        Collector::update_custom_levels(self.tracers.levels);
        Collector::set_metrics(self.metrics);
        Collector::reload();
    }
}

impl TelemetrySubscriberType {
    pub fn spawn(self, builder: SubscriberBuilder) {
        match self {
            TelemetrySubscriberType::ConsoleTracer(settings) => {
                spawn_console_tracer(builder, settings)
            }
            TelemetrySubscriberType::LogTracer(settings) => spawn_log_tracer(builder, settings),
            TelemetrySubscriberType::Webhook(settings) => spawn_webhook_tracer(builder, settings),
            TelemetrySubscriberType::OtelTracer(settings) => spawn_otel_tracer(builder, settings),
            #[cfg(unix)]
            TelemetrySubscriberType::JournalTracer(subscriber) => {
                tracers::journald::spawn_journald_tracer(builder, subscriber)
            }
            // SPDX-SnippetBegin
            // SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
            // SPDX-License-Identifier: AGPL-3.0-only
            TelemetrySubscriberType::StoreTracer(subscriber) => {
                tracers::store::spawn_store_tracer(builder, subscriber)
            } // SPDX-SnippetEnd
        }
    }
}

#[cfg(stalwart_tests)]
#[rustfmt::skip]
#[path = "../../../../../stalwart-tests/unit/common/src/telemetry/test_support.rs"]
mod test_support;
