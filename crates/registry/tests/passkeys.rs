// SPDX-License-Identifier: AGPL-3.0-only
use registry::{
    jmap::IntoValue,
    pickle::{Pickle, PickledStream, begin_block, end_block},
    schema::structs::{Credential, PasskeyCredential, UserAccount},
};

#[test]
fn old_accounts_read_without_changing_their_token_version() {
    // A pre-passkey UserAccount has 15 fields and no revocation epoch.
    let user = UserAccount::default();
    let mut bytes = vec![0];
    let block = begin_block(&mut bytes, 15, 4, 0);
    user.name.pickle(&mut bytes);
    user.domain_id.pickle(&mut bytes);
    user.credentials.pickle(&mut bytes);
    user.created_at.pickle(&mut bytes);
    user.member_group_ids.pickle(&mut bytes);
    user.roles.pickle(&mut bytes);
    user.permissions.pickle(&mut bytes);
    user.quotas.pickle(&mut bytes);
    user.aliases.pickle(&mut bytes);
    user.locale.pickle(&mut bytes);
    user.encryption_at_rest.pickle(&mut bytes);
    end_block(&mut bytes, block);
    let restored = UserAccount::unpickle(&mut PickledStream::new(&bytes).unwrap()).unwrap();
    assert_eq!(restored, user);
    assert_eq!(restored.credential_revision, 0);
}

#[test]
fn passkeys_and_revocation_epoch_survive_storage_without_public_key_disclosure() {
    let key = PasskeyCredential {
        credential_id: 7u32.into(),
        name: "Security key".into(),
        rp_id: "mail.example.org".into(),
        user_handle: "opaque-uuid".into(),
        credential_key: "opaque-credential-id".into(),
        public_key: "serialized-library-key-and-counters".into(),
        ..Default::default()
    };
    let mut user = UserAccount {
        credential_revision: 42,
        ..Default::default()
    };
    user.credentials.push(Credential::Passkey(key.clone()));
    let mut bytes = vec![0];
    user.pickle(&mut bytes);
    assert_eq!(
        UserAccount::unpickle(&mut PickledStream::new(&bytes).unwrap()).unwrap(),
        user
    );
    assert_eq!(user.next_credential_id(), 8);
    let public = serde_json::to_value(key.into_value()).unwrap();
    assert_eq!(public["@type"], "Passkey");
    assert_eq!(public["name"], "Security key");
    for private in ["publicKey", "credentialKey", "userHandle", "rpId"] {
        assert!(public.get(private).is_none());
    }
    assert!(
        !serde_json::to_string(&user)
            .unwrap()
            .contains("credentialRevision")
    );
}
