import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { gunzipSync } from 'node:zlib';
import { createPermissionCheckers } from '../src/lib/permissions';
import { isLinkAccessible } from '../src/lib/layout';
import { resolveItemActions } from '../src/components/lists/listActions';
import { useAccountStore } from '../src/stores/accountStore';
import type { Schema } from '../src/types/schema';

const schema: Schema = JSON.parse(gunzipSync(readFileSync('../resources/schema/schema.json.gz')).toString());

describe('permission-based access without editions', () => {
    it('accepts the server account response without an edition', () => {
        useAccountStore.getState().setAccountInfo(['liveTracing'], 'en');
        expect(useAccountStore.getState().permissions).toContain('liveTracing');
        expect(useAccountStore.getState()).not.toHaveProperty('edition');
    });
    it('permits live tracing when authorized and denies it otherwise', () => {
        for (const permissions of [[], ['liveTracing']]) {
            const checks = createPermissionCheckers(permissions);
            expect(isLinkAccessible(schema, 'CustomComponent/LiveTracing', checks.canGet, checks.hasPermission)).toBe(
                permissions.length > 0,
            );
        }
    });
    it('preserves branding permissions for settings and row actions', () => {
        const action = [{ type: 'view' as const, objectName: 'x:Branding', label: 'Branding' }];
        for (const permissions of [[], ['sysBrandingGet']]) {
            const checks = createPermissionCheckers(permissions);
            expect(isLinkAccessible(schema, 'x:Branding', checks.canGet, checks.hasPermission)).toBe(
                permissions.length > 0,
            );
            expect(resolveItemActions(schema, action, 'sysAccount', checks)).toHaveLength(permissions.length);
        }
    });
    it('exposes passkeys through the account credential permission', () => {
        for (const permissions of [[], ['sysAccountPasswordGet'], ['sysAccountPasswordUpdate']]) {
            const checks = createPermissionCheckers(permissions);
            expect(isLinkAccessible(schema, 'CustomComponent/Passkeys', checks.canGet, checks.hasPermission)).toBe(
                permissions.includes('sysAccountPasswordGet'),
            );
        }
    });
    it('has no activation form or edition feature metadata in the shipped schema', () => {
        expect(schema.fields['x:Branding'].properties).toHaveProperty('logo');
        expect(Object.keys(schema.fields['x:Branding'].properties)).toEqual(['logo']);
        expect(JSON.stringify(schema)).not.toMatch(/licenseKey|license\.stalw\.art|"enterprise":/);
    });
});
