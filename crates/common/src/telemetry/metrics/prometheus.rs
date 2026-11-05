/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 */

use crate::{Server, config::telemetry::PrometheusMetrics};
use std::sync::atomic::Ordering;
use telemetry::prometheus::{PromFormat, text::write_prometheus};

const SIZE_HINT_MARGIN: usize = 1024;

impl Server {
    pub fn export_prometheus_metrics(
        &self,
        prometheus: &PrometheusMetrics,
        format: PromFormat,
    ) -> Vec<u8> {
        self.inner.refresh_live_gauges();

        let mut out =
            Vec::with_capacity(prometheus.size_hint.load(Ordering::Relaxed) + SIZE_HINT_MARGIN);
        write_prometheus(&mut out, format);
        prometheus.size_hint.store(out.len(), Ordering::Relaxed);
        out
    }
}
