// SPDX-License-Identifier: AGPL-3.0-only

use crate::{
    jmap::{IntoValue, JmapValue, ValueOptions},
    pickle::{Pickle, PickledStream},
    schema::{prelude::Property, structs::PasskeyCredential},
};

impl Pickle for PasskeyCredential {
    fn pickle(&self, out: &mut Vec<u8>) {
        let block = crate::pickle::begin_block(out, 8, 0, 0);
        self.credential_id.pickle(out);
        self.name.pickle(out);
        self.rp_id.pickle(out);
        self.user_handle.pickle(out);
        self.credential_key.pickle(out);
        self.public_key.pickle(out);
        self.created_at.pickle(out);
        self.last_used_at.pickle(out);
        crate::pickle::end_block(out, block);
    }
    fn unpickle(stream: &mut PickledStream<'_>) -> Option<Self> {
        let block = stream.open_block()?;
        if !block.has(7) {
            return None;
        }
        let value = Self {
            credential_id: Pickle::unpickle(stream)?,
            name: Pickle::unpickle(stream)?,
            rp_id: Pickle::unpickle(stream)?,
            user_handle: Pickle::unpickle(stream)?,
            credential_key: Pickle::unpickle(stream)?,
            public_key: Pickle::unpickle(stream)?,
            created_at: Pickle::unpickle(stream)?,
            last_used_at: Pickle::unpickle(stream)?,
        };
        stream.close_block(block);
        Some(value)
    }
}
impl IntoValue for PasskeyCredential {
    fn into_value(self) -> JmapValue<'static> {
        self.into_value_with(ValueOptions::default())
    }
    fn into_value_with(self, _: ValueOptions) -> JmapValue<'static> {
        let mut map = jmap_tools::Map::with_capacity(5);
        map.insert_unchecked(Property::Type, JmapValue::Str("Passkey".into()));
        map.insert_unchecked(Property::CredentialId, self.credential_id.into_value());
        map.insert_unchecked(Property::Name, self.name.into_value());
        map.insert_unchecked(Property::CreatedAt, self.created_at.into_value());
        map.insert_unchecked(Property::LastUsedAt, self.last_used_at.into_value());
        JmapValue::Object(map)
    }
}
