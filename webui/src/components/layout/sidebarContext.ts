/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 */

import { createContext, useContext } from 'react';

export const SIDEBAR_ID = 'app-sidebar';

export interface SidebarNavContextValue {
    sectionName: string;
    pathname: string;
    activeViewName: string | undefined;
    isVisible: (viewName: string) => boolean;
}

export const SidebarNavContext = createContext<SidebarNavContextValue | null>(null);

export function useSidebarNav(): SidebarNavContextValue {
    const ctx = useContext(SidebarNavContext);
    if (!ctx) throw new Error('useSidebarNav must be used inside SidebarNavContext');
    return ctx;
}
