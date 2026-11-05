/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 */

import { create } from 'zustand';

export interface AccountState {
    permissions: string[];
    locale: string;

    setAccountInfo: (permissions: string[], locale: string) => void;
    hasPermission: (perm: string) => boolean;
    hasObjectPermission: (prefix: string, action: 'Get' | 'Query' | 'Create' | 'Update' | 'Destroy') => boolean;
}

export const useAccountStore = create<AccountState>()((set, get) => ({
    permissions: [],
    locale: 'en',

    setAccountInfo: (permissions, locale) => {
        set({ permissions, locale });
    },

    hasPermission: (perm) => {
        return get().permissions.includes(perm);
    },

    hasObjectPermission: (prefix, action) => {
        return get().permissions.includes(`${prefix}${action}`);
    },
}));
