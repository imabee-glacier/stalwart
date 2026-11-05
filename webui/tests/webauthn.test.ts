// SPDX-License-Identifier: AGPL-3.0-only
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
    createPasskey,
    decodeBase64Url,
    encodeBase64Url,
    passkeyError,
    authenticateWithPasskey,
} from '../src/features/passkeys/webauthn';

afterEach(() => vi.unstubAllGlobals());
describe('WebAuthn browser boundary', () => {
    it('preserves binary values including URL-unsafe bytes', () => {
        const bytes = new Uint8Array([0, 127, 128, 251, 255]).buffer;
        const encoded = encodeBase64Url(bytes);
        expect(encoded).not.toMatch(/[+/=]/);
        expect(decodeBase64Url(encoded)).toEqual(bytes);
        expect(() => decodeBase64Url('invalid+input')).toThrow();
    });
    it('decodes registration options and serializes platform credentials without toJSON', async () => {
        const bytes = new Uint8Array([1, 2, 255]).buffer;
        const create = vi.fn(async () => ({
            type: 'public-key',
            id: 'id',
            rawId: bytes,
            response: { clientDataJSON: bytes, attestationObject: bytes, getTransports: () => ['internal'] },
            getClientExtensionResults: () => ({ credProps: { rk: true } }),
        }));
        vi.stubGlobal('navigator', { credentials: { create } });
        const controller = new AbortController();
        const result = await createPasskey(
            {
                publicKey: {
                    challenge: 'AQL_',
                    user: { id: 'AQL_', name: 'a', displayName: 'a' },
                    excludeCredentials: [{ id: 'AQL_', type: 'public-key' }],
                },
            },
            controller.signal,
        );
        expect(create.mock.calls[0]).toEqual([
            {
                signal: controller.signal,
                publicKey: {
                    challenge: bytes,
                    user: { id: bytes, name: 'a', displayName: 'a' },
                    excludeCredentials: [{ id: bytes, type: 'public-key' }],
                },
            },
        ]);
        expect(result).toMatchObject({
            rawId: 'AQL_',
            response: { attestationObject: 'AQL_', transports: ['internal'] },
            clientExtensionResults: { credProps: { rk: true } },
        });
    });
    it('serializes assertions with an absent user handle and forwards cancellation', async () => {
        const bytes = new Uint8Array([1]).buffer;
        const get = vi.fn(async () => ({
            type: 'public-key',
            id: 'id',
            rawId: bytes,
            response: { clientDataJSON: bytes, authenticatorData: bytes, signature: bytes, userHandle: null },
            getClientExtensionResults: () => ({}),
        }));
        vi.stubGlobal('navigator', { credentials: { get } });
        expect(await authenticateWithPasskey({ publicKey: { challenge: 'AQ' } })).toMatchObject({
            response: { userHandle: null, signature: 'AQ' },
        });
        expect(passkeyError(new DOMException('', 'NotAllowedError'))).toMatch(/cancelled or timed out/);
    });
});
