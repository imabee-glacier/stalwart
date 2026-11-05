// SPDX-License-Identifier: AGPL-3.0-only

export interface CeremonyOptions {
    challengeId: string;
    options: { publicKey: Record<string, unknown> };
}
export function passkeysSupported(): boolean {
    return window.isSecureContext && typeof PublicKeyCredential !== 'undefined' && !!navigator.credentials;
}
export function decodeBase64Url(value: string): ArrayBuffer {
    if (!/^[A-Za-z0-9_-]*$/.test(value)) throw new Error('Invalid WebAuthn binary value');
    const bytes = Uint8Array.from(atob(value.replace(/-/g, '+').replace(/_/g, '/')), (c) => c.charCodeAt(0));
    return bytes.buffer;
}
export function encodeBase64Url(value: ArrayBuffer): string {
    return btoa(Array.from(new Uint8Array(value), (v) => String.fromCharCode(v)).join(''))
        .replace(/\+/g, '-')
        .replace(/\//g, '_')
        .replace(/=+$/, '');
}
function descriptors(value: unknown): PublicKeyCredentialDescriptor[] | undefined {
    if (!Array.isArray(value)) return undefined;
    return value.map((c) => ({ ...c, id: decodeBase64Url(c.id) }));
}
function serializeCredential(credential: Credential | null): Record<string, unknown> {
    if (!credential || credential.type !== 'public-key') throw new Error('No passkey was selected');
    const key = credential as PublicKeyCredential;
    const response = key.response;
    const data: Record<string, unknown> = { clientDataJSON: encodeBase64Url(response.clientDataJSON) };
    if ('attestationObject' in response) {
        const registration = response as AuthenticatorAttestationResponse;
        data.attestationObject = encodeBase64Url(registration.attestationObject);
        data.transports = registration.getTransports?.();
    } else {
        const assertion = response as AuthenticatorAssertionResponse;
        data.authenticatorData = encodeBase64Url(assertion.authenticatorData);
        data.signature = encodeBase64Url(assertion.signature);
        data.userHandle = assertion.userHandle ? encodeBase64Url(assertion.userHandle) : null;
    }
    return {
        id: key.id,
        rawId: encodeBase64Url(key.rawId),
        type: key.type,
        response: data,
        clientExtensionResults: key.getClientExtensionResults(),
    };
}
export async function createPasskey(
    options: CeremonyOptions['options'],
    signal?: AbortSignal,
): Promise<Record<string, unknown>> {
    const raw = options.publicKey;
    const user = raw.user as PublicKeyCredentialUserEntity & { id: string };
    const publicKey = {
        ...raw,
        challenge: decodeBase64Url(raw.challenge as string),
        user: { ...user, id: decodeBase64Url(user.id) },
        excludeCredentials: descriptors(raw.excludeCredentials),
    } as PublicKeyCredentialCreationOptions;
    return serializeCredential(await navigator.credentials.create({ publicKey, signal }));
}
export async function authenticateWithPasskey(
    options: CeremonyOptions['options'],
    signal?: AbortSignal,
): Promise<Record<string, unknown>> {
    const raw = options.publicKey;
    const publicKey = {
        ...raw,
        challenge: decodeBase64Url(raw.challenge as string),
        allowCredentials: descriptors(raw.allowCredentials),
    } as PublicKeyCredentialRequestOptions;
    return serializeCredential(await navigator.credentials.get({ publicKey, signal }));
}
export function passkeyError(error: unknown): string | null {
    if (!(error instanceof DOMException)) return null;
    if (error.name === 'AbortError' || error.name === 'NotAllowedError')
        return 'The passkey request was cancelled or timed out. You can try again.';
    if (error.name === 'InvalidStateError')
        return 'This passkey may already be registered. Try another device or security key.';
    if (error.name === 'SecurityError')
        return 'Open the account portal at your server’s configured secure address to use passkeys.';
    if (error.name === 'NotSupportedError')
        return 'This authenticator cannot create the requested passkey. Try another device or security key.';
    return null;
}
