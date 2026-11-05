/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 *
 */

use common::ai::SpamAiWebSearch;
use serde_json::value::RawValue;

pub fn web_context(search: SpamAiWebSearch) -> Option<Box<RawValue>> {
    match search {
        SpamAiWebSearch::Disabled => None,
    }
}
