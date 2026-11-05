// SPDX-License-Identifier: AGPL-3.0-only
// The parent smoke test supplies a disposable local server and test-only credentials.
import { chromium } from 'playwright';
import assert from 'node:assert/strict';
const base = process.env.STALWART_TEST_BASE_URL;
assert(base?.startsWith('http://127.0.0.1:'));
const browser = await chromium.launch({ headless: true });
try {
    const context = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
    await context.route('**/*', (route) => {
        if (
            !route
                .request()
                .url()
                .startsWith(base + '/')
        )
            return route.abort();
        return route.continue({
            headers: { ...route.request().headers(), authorization: process.env.STALWART_TEST_AUTHORIZATION },
        });
    });
    // Bypass the unrelated OAuth login flow. Every API request still authenticates
    // against the real local server using its synthetic recovery administrator.
    await context.addInitScript(() =>
        sessionStorage.setItem(
            'stalwart-auth',
            JSON.stringify({
                state: {
                    accessToken: 'browser-smoke',
                    tokenExpiresAt: Date.now() + 3600000,
                    tokenEndpoint: '',
                    tokenLifetime: 3600000,
                },
                version: 0,
            }),
        ),
    );
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await page.goto(base + '/admin/Settings/x:Branding', { waitUntil: 'networkidle' });
    await page.getByRole('heading', { name: 'Branding', exact: true, level: 1 }).waitFor({ timeout: 15000 });
    await page.getByRole('textbox', { name: /Default Logo/ }).waitFor();
    const logo = page.getByRole('textbox', { name: /Default Logo/ });
    await logo.fill('https://example.org/validation-logo.png');
    await logo.blur();
    const saved = page.waitForResponse(
        (response) => response.request().method() === 'POST' && /\/jmap\/?$/.test(response.url()),
    );
    await page.getByRole('button', { name: 'Save', exact: true }).click();
    const result = await (await saved).json();
    for (const call of result.methodResponses ?? []) {
        assert.notEqual(call[0], 'error', JSON.stringify(call));
        assert.equal(Object.keys(call[1].notUpdated ?? {}).length, 0, JSON.stringify(call));
    }
    await page.reload({ waitUntil: 'networkidle' });
    await logo.waitFor();
    assert.equal(await logo.inputValue(), 'https://example.org/validation-logo.png');
    const text = await page.locator('body').innerText();
    assert(!/Try Enterprise|Unlock Enterprise|License Key|requires an Enterprise license/i.test(text));
    assert.equal(errors.length, 0, errors.join('\n'));
    await page.screenshot({
        path: process.env.STALWART_TEST_SCREENSHOT || '/tmp/stalwart-dashboard.png',
        fullPage: true,
    });
    console.log('PASS: browser saves and reloads Branding without activation or purchase controls');
} finally {
    await browser.close();
}
