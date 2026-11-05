/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 */

import { useSchemaStore } from '@/stores/schemaStore';
import { usePermissionCheckers } from '@/hooks/usePermissions';
import { isLinkVisible, visibleLayouts } from '@/lib/layout';
import { sectionLandingLink } from '@/lib/lastVisited';
import type { Layout } from '@/types/schema';
import { viewPath } from './navTree';

export function useLayoutNavigation() {
    const schema = useSchemaStore((s) => s.schema);
    const { canGet, hasPermission } = usePermissionCheckers();

    const layouts = schema ? visibleLayouts(schema, canGet, hasPermission) : [];

    return {
        layouts,
        isVisible: (viewName: string) => !schema || isLinkVisible(schema, viewName, canGet, hasPermission),
        landingPath: (layout: Layout): string | null => {
            if (!schema) return null;
            const link = sectionLandingLink(schema, layout, canGet, hasPermission);
            return link ? viewPath(layout.name, link) : null;
        },
    };
}
