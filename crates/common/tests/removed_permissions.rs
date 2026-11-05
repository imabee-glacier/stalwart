// SPDX-License-Identifier: AGPL-3.0-only

use common::auth::{
    AccessToken, PermissionsGroup,
    permissions::{BuildPermissions, PermissionsListBuilder},
};
use registry::{
    schema::{enums::Permission, structs::PermissionsList},
    types::{EnumImpl, map::Map},
};
use utils::map::vec_map::VecMap;

const RETIRED: [Permission; 6] = [
    Permission::RetiredPop3Authenticate,
    Permission::RetiredPop3List,
    Permission::RetiredPop3Uidl,
    Permission::RetiredPop3Stat,
    Permission::RetiredPop3Retr,
    Permission::RetiredPop3Dele,
];

#[test]
fn admin_holds_every_active_permission_and_no_retired_ones() {
    let admin = AccessToken::new_admin();
    let held = admin.permissions();
    assert_eq!(held.len(), Permission::ALL.len());
    assert!(Permission::ALL.iter().all(|p| admin.has_permission(*p)));
    assert!(RETIRED.iter().all(|p| !admin.has_permission(*p)));
    assert!(held.iter().all(|p| !p.is_retired()));
}

#[test]
fn stored_roles_with_retired_permissions_are_ignored() {
    let list = PermissionsList {
        enabled_permissions: Map::new(
            [Permission::ImapAuthenticate]
                .into_iter()
                .chain(RETIRED)
                .collect(),
        ),
        disabled_permissions: Map::new(RETIRED.to_vec()),
    };
    let group = PermissionsGroup::from(&list);
    assert_eq!(
        group.enabled.build_permissions_list(),
        vec![Permission::ImapAuthenticate]
    );
    assert!(group.disabled.is_empty());

    let mut map = VecMap::new();
    map.set(Permission::RetiredPop3Retr, true);
    map.set(Permission::RetiredPop3Dele, false);
    let group = PermissionsGroup::from(&map);
    assert!(group.enabled.is_empty() && group.disabled.is_empty());

    // An administrator can still grant a legacy role: its retired IDs are
    // dropped instead of failing as permissions the grantor does not hold.
    let admin = AccessToken::new_admin();
    let requested = common::auth::Permissions::from_permission(&RETIRED);
    assert!(admin.can_grant_permissions(requested).is_ok());
}
