// SPDX-License-Identifier: AGPL-3.0-only

use registry::{
    pickle::{Pickle, PickledStream, begin_block, end_block},
    schema::structs::{Branding, SecretKeyOptional, SecretKeyValue},
    types::ObjectImpl,
};

#[test]
fn legacy_activation_records_preserve_branding_and_discard_credentials() {
    let mut bytes = vec![0];
    let block = begin_block(&mut bytes, 3, 1, 1);
    for secret in ["old-api-key", "expired-license-key"] {
        SecretKeyOptional::Value(SecretKeyValue {
            secret: secret.into(),
        })
        .pickle(&mut bytes);
    }
    "https://example.org/logo.png"
        .to_string()
        .pickle(&mut bytes);
    end_block(&mut bytes, block);

    let branding = Branding::unpickle(&mut PickledStream::new(&bytes).unwrap()).unwrap();
    assert_eq!(
        branding.logo.as_deref(),
        Some("https://example.org/logo.png")
    );
    let json = serde_json::to_value(&branding).unwrap();
    assert_eq!(json.as_object().unwrap().len(), 1);
    assert!(json.get("apiKey").is_none());
    assert!(json.get("licenseKey").is_none());

    let mut rewritten = vec![Branding::VERSION];
    branding.pickle(&mut rewritten);
    let mut stream = PickledStream::new(&rewritten).unwrap();
    assert_eq!(stream.version(), 1);
    let block = stream.open_block().unwrap();
    assert!(block.has(0));
    assert!(
        !block.has(1),
        "new records must not contain credential slots"
    );
    assert!(
        !rewritten
            .windows(b"old-api-key".len())
            .any(|s| s == b"old-api-key")
    );
    assert!(
        !rewritten
            .windows(b"expired-license-key".len())
            .any(|s| s == b"expired-license-key")
    );
    assert_eq!(
        Branding::unpickle(&mut PickledStream::new(&rewritten).unwrap()).unwrap(),
        branding
    );
}

#[test]
fn fresh_branding_needs_no_activation() {
    let branding = Branding::default();
    let mut bytes = vec![Branding::VERSION];
    branding.pickle(&mut bytes);
    assert_eq!(
        Branding::unpickle(&mut PickledStream::new(&bytes).unwrap()).unwrap(),
        branding
    );
}

#[test]
fn branding_keeps_existing_storage_and_permission_ids() {
    use registry::{schema::prelude::ObjectType, types::EnumImpl};
    assert_eq!(ObjectType::from_id(40), Some(ObjectType::Branding));
    assert_eq!(ObjectType::Branding.as_str(), "Branding");
    use registry::schema::enums::Permission;
    assert_eq!(Permission::from_id(363), Some(Permission::SysBrandingGet));
    assert_eq!(
        Permission::from_id(364),
        Some(Permission::SysBrandingUpdate)
    );
}
