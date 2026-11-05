/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 *
 */

#[cfg(any(feature = "postgres", feature = "mysql"))]
pub mod read_replica;
pub mod sharded_blob;
pub mod sharded_lookup;
