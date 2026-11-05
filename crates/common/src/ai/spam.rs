/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 *
 */

use super::{AiClassifier, AiVerdict, BreakerPolicy, PreparedClassifier};
pub use crate::config::mailstore::spamfilter::AI_TAG_PREFIX;
use crate::expr::if_block::IfBlock;
use compact_str::CompactString;
use std::{sync::Arc, time::Duration};

pub struct SpamAiConfig {
    pub mode: SpamAiMode,
    pub condition: IfBlock,
    pub tiers: Box<[SpamAiTier]>,
    pub classifier: Arc<AiClassifier>,
    pub category_tags: Box<[[CompactString; 3]]>,
    pub signal_tags: Box<[CompactString]>,
    pub buckets: SpamAiBuckets,
    pub add_header: bool,
    pub input: SpamAiInput,
    pub deadline: Duration,
    pub breaker: BreakerPolicy,
    pub learn: Option<SpamAiLearn>,
    pub web_search: SpamAiWebSearch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpamAiMode {
    Automatic,
    Shadow,
}

pub struct SpamAiTier {
    pub classifier: PreparedClassifier,
    pub escalate_below: f32,
    pub escalate_on: Box<[u8]>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpamAiBuckets {
    pub high: f32,
    pub medium: f32,
    pub uncertain_below: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpamAiInput {
    pub max_input_chars: usize,
    pub max_links: usize,
    pub max_attachments: usize,
    pub trim_quotes: bool,
    pub include_auth: bool,
    pub include_links: bool,
    pub include_attachments: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpamAiLearn {
    pub min_probability: f32,
    pub sample_rate: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpamAiWebSearch {
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpamAiBucket {
    High,
    Medium,
    Low,
}

pub const AI_TAG_UNCERTAIN: &str = "AI_UNCERTAIN";
pub const AI_TAG_ERROR: &str = "AI_ERROR";
pub const AI_SIGNAL_THRESHOLD: f32 = 0.5;

impl SpamAiBuckets {
    pub fn bucket(&self, probability: f32) -> SpamAiBucket {
        if probability >= self.high {
            SpamAiBucket::High
        } else if probability >= self.medium {
            SpamAiBucket::Medium
        } else {
            SpamAiBucket::Low
        }
    }
}

impl SpamAiBucket {
    pub fn index(self) -> usize {
        match self {
            SpamAiBucket::High => 0,
            SpamAiBucket::Medium => 1,
            SpamAiBucket::Low => 2,
        }
    }
}

impl SpamAiConfig {
    pub fn is_uncertain(&self, verdict: &AiVerdict) -> bool {
        verdict.confidence < self.buckets.uncertain_below
    }

    pub fn should_escalate(&self, tier: &SpamAiTier, verdict: &AiVerdict) -> bool {
        self.is_uncertain(verdict)
            || verdict.confidence < tier.escalate_below
            || self.buckets.bucket(verdict.probability) == SpamAiBucket::Low
            || tier.escalate_on.contains(&verdict.category)
    }

    pub fn verdict_tags(&self, verdict: &AiVerdict, tags: &mut Vec<CompactString>) {
        if self.is_uncertain(verdict) {
            tags.push(CompactString::const_new(AI_TAG_UNCERTAIN));
        } else if let Some(tag) =
            self.category_tag(verdict.category, self.buckets.bucket(verdict.probability))
        {
            tags.push(tag.clone());
        }
        for (signal, probability) in verdict.signals.iter() {
            if *probability >= AI_SIGNAL_THRESHOLD
                && let Some(tag) = self.signal_tag(*signal)
            {
                tags.push(tag.clone());
            }
        }
    }

    pub fn category_name(&self, category: u8) -> &str {
        self.classifier
            .categories
            .get(usize::from(category))
            .map_or("unknown", |category| category.name.as_ref())
    }

    pub fn category_tag(&self, category: u8, bucket: SpamAiBucket) -> Option<&CompactString> {
        self.category_tags
            .get(usize::from(category))
            .and_then(|tags| tags.get(bucket.index()))
    }

    pub fn signal_tag(&self, signal: u8) -> Option<&CompactString> {
        self.signal_tags.get(usize::from(signal))
    }
}
