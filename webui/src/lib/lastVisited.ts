/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 */

import {
    findFirstAccessibleLinkInLayout,
    findFirstVisibleLinkInLayout,
    isLinkAccessible,
    layoutContainsLink,
    type CanGet,
    type HasPermission,
} from '@/lib/layout';
import type { Layout, Schema } from '@/types/schema';

const STORAGE_KEY = 'stalwart-last-visited';

function readAll(): Record<string, unknown> {
    try {
        const raw = localStorage.getItem(STORAGE_KEY);
        const parsed: unknown = raw ? JSON.parse(raw) : null;
        return parsed && typeof parsed === 'object' && !Array.isArray(parsed)
            ? (parsed as Record<string, unknown>)
            : {};
    } catch {
        return {};
    }
}

export function rememberLastVisited(section: string, viewName: string): void {
    try {
        const all = readAll();
        if (all[section] === viewName) return;
        localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...all, [section]: viewName }));
    } catch {
        return;
    }
}

export function sectionLandingLink(
    schema: Schema,
    layout: Layout,
    canGet: CanGet,
    hasPerm?: HasPermission,
): string | null {
    const last = readAll()[layout.name];
    if (
        typeof last === 'string' &&
        layoutContainsLink(layout, last) &&
        isLinkAccessible(schema, last, canGet, hasPerm)
    ) {
        return last;
    }
    return (
        findFirstAccessibleLinkInLayout(schema, layout, canGet, hasPerm) ??
        findFirstVisibleLinkInLayout(schema, layout, canGet, hasPerm)
    );
}
