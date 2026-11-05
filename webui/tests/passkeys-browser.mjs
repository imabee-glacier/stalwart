// SPDX-License-Identifier: AGPL-3.0-only
// Only runs against a disposable SQLite server started by test_open_source_server.py.
import * as OTPAuth from 'otpauth';
import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import { randomBytes, createHash } from 'node:crypto';
const base = process.env.STALWART_TEST_BASE_URL;
assert.match(base, /^http:\/\/localhost:\d+$/);
const admin = process.env.STALWART_TEST_AUTHORIZATION;
async function api(path, body, authorization = admin) {
    const response = await fetch(base + path, {
        method: body ? 'POST' : 'GET',
        headers: { authorization, origin: base, 'content-type': 'application/json' },
        body: body ? JSON.stringify(body) : undefined,
    });
    const data = await response.json();
    assert(response.ok, JSON.stringify(data));
    return data;
}
const session = await api('/jmap/session');

const accountId = session.primaryAccounts['urn:stalwart:jmap'] ?? Object.keys(session.accounts)[0];
async function create(type, value) {
    const response = await api('/jmap', {
        using: ['urn:ietf:params:jmap:core', 'urn:stalwart:jmap'],
        methodCalls: [[`x:${type}/set`, { accountId, create: { test: value } }, 'c']],
    });
    assert(response.methodResponses[0][1].created?.test, JSON.stringify(response));
    return response.methodResponses[0][1].created.test.id;
}
const domainId = await create('Domain', { name: 'passkey.test' });
const password = randomBytes(24).toString('base64url');
const userId = await create('Account', {
    '@type': 'User',
    name: 'alice',
    domainId,
    permissions: {
        '@type': 'Replace',
        enabledPermissions: { authenticate: true, sysAccountPasswordGet: true, sysAccountPasswordUpdate: true },
    },
    credentials: { 0: { '@type': 'Password', secret: password } },
});
const authorization = 'Basic ' + Buffer.from('alice@passkey.test:' + password).toString('base64');
console.log('PASS: created native user', userId);
const clientId = 'passkey-browser-test';
const redirectUri = base + '/test-callback';
await create('OAuthClient', {
    clientId,
    redirectUris: { [redirectUri]: true },
    grantTypes: { authorizationCode: true, refreshToken: true, deviceCode: true },
    scopes: { security: true },
});
const browser = await chromium.launch({ headless: true });
try {
    const context = await browser.newContext();
    const page = await context.newPage();
    page.setDefaultTimeout(15000);
    const cdp = await context.newCDPSession(page);
    await cdp.send('WebAuthn.enable');
    const { authenticatorId } = await cdp.send('WebAuthn.addVirtualAuthenticator', {
        options: {
            protocol: 'ctap2',
            transport: 'internal',
            hasResidentKey: true,
            hasUserVerification: true,
            isUserVerified: true,
            automaticPresenceSimulation: true,
        },
    });
    let browserAuthorization = authorization;
    const captured = [];
    const loginProofs = [];
    await context.route('**/*', (route) => {
        if (
            route.request().method() === 'POST' &&
            route.request().url().endsWith('/api/auth') &&
            route.request().postDataJSON()?.type === 'passkeyVerify'
        )
            loginProofs.push(route.request().postDataJSON());
        if (route.request().method() === 'POST' && route.request().url().includes('/passkeys/'))
            captured.push({ path: new URL(route.request().url()).pathname, body: route.request().postDataJSON() });
        if (route.request().url().startsWith(redirectUri))
            return route.fulfill({ status: 200, contentType: 'text/html', body: '<title>OAuth test callback</title>' });
        if (
            !route
                .request()
                .url()
                .startsWith(base + '/')
        )
            return route.abort();
        return route.continue({ headers: { ...route.request().headers(), authorization: browserAuthorization } });
    });
    await context.addInitScript(() =>
        sessionStorage.setItem(
            'stalwart-auth',
            JSON.stringify({
                state: {
                    accessToken: 'test',
                    tokenExpiresAt: Date.now() + 3600000,
                    tokenEndpoint: '',
                    tokenLifetime: 3600000,
                },
                version: 0,
            }),
        ),
    );
    page.on('pageerror', (err) => console.error('PAGE ERROR', err));
    await page.goto(base + '/admin/Account/CustomComponent/Passkeys', { waitUntil: 'networkidle' });
    await page.getByRole('heading', { name: 'Passkeys', exact: true }).waitFor();
    await page.getByRole('button', { name: 'Add passkey', exact: true }).click();
    await page.getByLabel('Passkey name').fill('Work laptop');
    await page.getByLabel('Current password').fill(password);
    await page.getByRole('button', { name: 'Continue', exact: true }).click();
    await page.getByRole('heading', { name: 'Work laptop', exact: true }).waitFor({ timeout: 15000 });
    console.log('PASS: browser creates and lists a real passkey');
    const registration = captured.find((c) => c.path.endsWith('/register/finish'));
    assert(registration);
    const replay = await fetch(base + registration.path, {
        method: 'POST',
        headers: { authorization, origin: base, 'content-type': 'application/json' },
        body: JSON.stringify(registration.body),
    });
    assert(!replay.ok, 'registration replay accepted');
    const list = await api('/api/account/passkeys', null, authorization);
    assert.equal(list.list.length, 1);
    assert.deepEqual(Object.keys(list.list[0]).sort(), ['createdAt', 'currentOrigin', 'id', 'lastUsedAt', 'name']);
    await page.getByRole('button', { name: 'Rename Work laptop' }).click();
    await page.getByLabel('Passkey name').fill('Personal passkey');
    await page.getByRole('button', { name: 'Save', exact: true }).click();
    await page.getByRole('heading', { name: 'Personal passkey', exact: true }).waitFor();
    console.log('PASS: rename persists, key material stays private, registration replay rejected');
    // Test the actual hosted OAuth login UI with its normal consent and PKCE exchange.
    const verifier = randomBytes(32).toString('base64url');
    const query = new URLSearchParams({
        client_id: clientId,
        redirect_uri: redirectUri,
        response_type: 'code',
        scope: 'openid urn:stalwart:oauth:scope:security',
        state: 'passkey-state',
        code_challenge: createHash('sha256').update(verifier).digest('base64url'),
        code_challenge_method: 'S256',
        login_hint: 'alice@passkey.test',
    });
    await page.goto(base + '/login?' + query);
    await page.getByRole('button', { name: 'Sign in with passkey', exact: true }).click();
    await page.getByRole('button', { name: 'Allow', exact: true }).click();
    await page.waitForURL(base + '/test-callback?**');
    const callback = new URL(page.url());
    assert.equal(callback.searchParams.get('state'), 'passkey-state');
    const response = await fetch(base + '/auth/token', {
        method: 'POST',
        headers: { 'content-type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({
            grant_type: 'authorization_code',
            client_id: clientId,
            redirect_uri: redirectUri,
            code: callback.searchParams.get('code'),
            code_verifier: verifier,
        }),
    });
    const tokens = await response.json();
    assert(tokens.access_token, JSON.stringify(tokens));
    browserAuthorization = 'Bearer ' + tokens.access_token;
    assert.equal((await api('/api/account/passkeys', null, browserAuthorization)).list.length, 1);
    assert.equal((await api('/auth/userinfo', null, 'Bearer ' + tokens.access_token)).sub, '1');
    const authReplay = await api('/api/auth', loginProofs[0]);
    assert.equal(authReplay.type, 'failure');
    console.log('PASS: passkey sign-in completes OAuth consent, PKCE token exchange, and userinfo; replay rejected');
    const deviceResponse = await fetch(base + '/auth/device', {
        method: 'POST',
        headers: { 'content-type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({ client_id: clientId, scope: 'openid' }),
    });
    const device = await deviceResponse.json();
    assert(device.device_code, JSON.stringify(device));
    await page.goto(base + '/device?code=' + encodeURIComponent(device.user_code));
    await page.getByRole('button', { name: 'Allow', exact: true }).click();
    await page.getByLabel('Username', { exact: true }).fill('alice@passkey.test');
    await page.getByRole('button', { name: 'Sign in with passkey', exact: true }).click();
    await page.getByRole('heading', { name: 'Device authorized', exact: true }).waitFor();
    const deviceTokenResponse = await fetch(base + '/auth/token', {
        method: 'POST',
        headers: { 'content-type': 'application/x-www-form-urlencoded' },
        body: new URLSearchParams({
            grant_type: 'urn:ietf:params:oauth:grant-type:device_code',
            client_id: clientId,
            device_code: device.device_code,
        }),
    });
    const deviceTokens = await deviceTokenResponse.json();
    assert(deviceTokens.access_token, JSON.stringify(deviceTokens));
    console.log('PASS: passkey device authorization issues tokens through the existing OAuth device grant');
    await page.goto(base + '/admin/Account/CustomComponent/Passkeys', { waitUntil: 'networkidle' });
    async function assertion() {
        const challenge = await api('/api/account/passkeys/verify/begin', {}, authorization);
        const credential = await page.evaluate(async (options) => {
            const decode = (value) =>
                Uint8Array.from(atob(value.replace(/-/g, '+').replace(/_/g, '/')), (c) => c.charCodeAt(0)).buffer;
            const encode = (value) =>
                btoa(String.fromCharCode(...new Uint8Array(value)))
                    .replace(/\+/g, '-')
                    .replace(/\//g, '_')
                    .replace(/=+$/, '');
            const key = await navigator.credentials.get({
                publicKey: { ...options.publicKey, challenge: decode(options.publicKey.challenge) },
            });
            return {
                id: key.id,
                rawId: encode(key.rawId),
                type: key.type,
                response: {
                    clientDataJSON: encode(key.response.clientDataJSON),
                    authenticatorData: encode(key.response.authenticatorData),
                    signature: encode(key.response.signature),
                    userHandle: encode(key.response.userHandle),
                },
                clientExtensionResults: key.getClientExtensionResults(),
            };
        }, challenge.options);
        return { challengeId: challenge.challengeId, credential };
    }
    async function rejected(path, body, headers = {}, auth = authorization) {
        const response = await fetch(base + path, {
            method: 'POST',
            headers: { authorization: auth, origin: base, 'content-type': 'application/json', ...headers },
            body: JSON.stringify(body),
        });
        assert(!response.ok, `${path} accepted an invalid request`);
    }
    const concurrent = await assertion();
    const competing = await Promise.all(
        [0, 1].map(() =>
            fetch(base + '/api/account/passkeys/verify/finish', {
                method: 'POST',
                headers: { authorization, origin: base, 'content-type': 'application/json' },
                body: JSON.stringify(concurrent),
            }),
        ),
    );
    assert.equal(competing.filter((response) => response.ok).length, 1, 'ceremony must have exactly one winner');
    await cdp.send('WebAuthn.setResponseOverrideBits', { authenticatorId, isBadUV: true });
    await rejected('/api/account/passkeys/verify/finish', await assertion());
    await cdp.send('WebAuthn.setResponseOverrideBits', { authenticatorId, isBadUV: false });
    const badSignature = await assertion();
    badSignature.credential.response.signature = Buffer.alloc(70).toString('base64url');
    await rejected('/api/account/passkeys/verify/finish', badSignature);
    const badOrigin = await assertion();
    await rejected('/api/account/passkeys/verify/finish', badOrigin, { origin: 'https://attacker.invalid' });
    // Origin rejection does not consume valid same-origin state.
    const stepUp = await api('/api/account/passkeys/verify/finish', badOrigin, authorization);
    assert(stepUp.authorization);
    await api('/api/account/passkeys/register/begin', { name: 'Second key', ...stepUp }, authorization);
    await rejected('/api/account/passkeys/register/begin', { name: 'Replayed proof', ...stepUp });
    await rejected('/api/account/passkeys/register/begin', { name: 'Wrong password', password: 'incorrect' });
    const otherId = await create('Account', {
        '@type': 'User',
        name: 'bob',
        domainId,
        permissions: {
            '@type': 'Replace',
            enabledPermissions: { authenticate: true, sysAccountPasswordGet: true, sysAccountPasswordUpdate: true },
        },
        credentials: { 0: { '@type': 'Password', secret: password } },
    });
    assert(otherId);
    const otherAuth = 'Basic ' + Buffer.from('bob@passkey.test:' + password).toString('base64');
    await rejected('/api/account/passkeys/verify/finish', await assertion(), {}, otherAuth);
    console.log(
        'PASS: concurrent replay, missing user verification, tampered signatures, foreign origins, wrong passwords, cross-account proofs, and step-up replay rejected',
    );
    async function setOtp(value) {
        const result = await api('/jmap', {
            using: ['urn:ietf:params:jmap:core', 'urn:stalwart:jmap'],
            methodCalls: [
                ['x:Account/set', { accountId, update: { [userId]: { 'credentials/0/otpUrl': value } } }, 'mfa'],
            ],
        });
        assert(Object.hasOwn(result.methodResponses[0][1].updated ?? {}, userId), JSON.stringify(result));
    }
    const totp = new OTPAuth.TOTP({
        issuer: 'Stalwart',
        label: 'alice',
        secret: OTPAuth.Secret.fromHex(randomBytes(20).toString('hex')),
    });
    await setOtp(totp.toString());
    await rejected('/api/account/passkeys/register/begin', { name: 'No OTP', password }, {}, browserAuthorization);
    await api(
        '/api/account/passkeys/register/begin',
        { name: 'Verified OTP', password, mfaToken: totp.generate() },
        browserAuthorization,
    );
    await setOtp(null);
    console.log('PASS: password step-up enforces configured MFA; scoped OAuth can manage passkeys');
    await page.screenshot({ path: '/tmp/stalwart-passkeys.png', fullPage: true });
    await page.evaluate(() => document.documentElement.classList.add('dark'));
    await page.screenshot({ path: '/tmp/stalwart-passkeys-dark.png', fullPage: true });
    await page.setViewportSize({ width: 390, height: 844 });
    const navigation = page.getByRole('button', { name: 'Toggle navigation' });
    if ((await navigation.getAttribute('aria-expanded')) === 'true') await navigation.click();
    await page.waitForFunction(() => document.querySelector('main').getBoundingClientRect().x < 30);
    await page.waitForFunction(() => document.documentElement.scrollWidth <= window.innerWidth);
    await page.getByRole('button', { name: 'Add passkey', exact: true }).click();
    await page.getByRole('dialog').waitFor();
    await page.screenshot({ path: '/tmp/stalwart-passkeys-mobile.png', fullPage: true, animations: 'disabled' });
    await page.getByRole('button', { name: 'Cancel', exact: true }).click();
    await page.setViewportSize({ width: 1280, height: 720 });
    await page.getByRole('button', { name: 'Remove Personal passkey', exact: true }).click();
    await page.getByRole('button', { name: 'Remove passkey', exact: true }).click();
    await page.waitForURL('**/login**');
    assert.equal((await api('/api/account/passkeys', null, authorization)).list.length, 0);
    const revoked = await fetch(base + '/auth/userinfo', {
        headers: { authorization: 'Bearer ' + tokens.access_token },
    });
    assert(!revoked.ok, 'token issued before passkey removal remained usable');
    console.log('PASS: passkey step-up removes the key, signs out the UI, and revokes existing OAuth tokens');
} finally {
    await browser.close();
}
