#!/usr/bin/env python3
"""Exercise a disposable server, its authenticated APIs, and optionally Webadmin.

Run from anywhere after building the SQLite binary. With --browser, first run
`npm ci --prefix webui` and `webui/node_modules/.bin/playwright install chromium`.
"""
# SPDX-License-Identifier: AGPL-3.0-only
import argparse
import base64
import gzip
import json
import os
from pathlib import Path
import secrets
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--browser', action='store_true')
    parser.add_argument('--bootstrap', action='store_true')
    parser.add_argument('--passkeys', action='store_true')
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/debug/stalwart')
    args = parser.parse_args()
    if (args.browser or args.passkeys) and args.bootstrap:
        parser.error('The Branding browser test requires the SQLite fixture; omit --bootstrap.')

    with socket.socket() as probe:
        probe.bind(('127.0.0.1', 0))
        port = probe.getsockname()[1]
    with tempfile.TemporaryDirectory(prefix='stalwart-smoke-') as directory:
        work = Path(directory)
        config = work / 'config.json'
        if not args.bootstrap:
            config.write_text(json.dumps({'@type': 'Sqlite', 'path': str(work / 'data.sqlite3'), 'poolWorkers': 2}))
        password = secrets.token_urlsafe(24)
        authorization = 'Basic ' + base64.b64encode(('admin:' + password).encode()).decode()
        env = {**os.environ, 'STALWART_HOSTNAME': 'localhost',
               'STALWART_RECOVERY_MODE_PORT': str(port), 'STALWART_RECOVERY_MODE': 'true',
               'STALWART_RECOVERY_ADMIN': 'admin:' + password}
        with (work / 'server.log').open('w') as log:
            process = subprocess.Popen([str(args.binary.resolve()), '--config', str(config)],
                                       env=env, stdout=log, stderr=log, cwd=ROOT)
            try:
                base = f'http://127.0.0.1:{port}'

                def request(path, authenticated=True):
                    req = urllib.request.Request(base + path)
                    if authenticated:
                        req.add_header('Authorization', authorization)
                    with urllib.request.urlopen(req, timeout=5) as response:
                        data = response.read()
                        if data.startswith(b'\x1f\x8b'):
                            data = gzip.decompress(data)
                        return response.status, data

                for _ in range(100):
                    if process.poll() is not None:
                        raise RuntimeError('Server stopped during startup')
                    try:
                        status, body = request('/admin/', False)
                        if status == 200:
                            break
                    except (urllib.error.URLError, TimeoutError):
                        time.sleep(.25)
                else:
                    raise RuntimeError('Dashboard startup timed out')
                assert b'<script' in body
                print('PASS: startup serves bundled dashboard without activation', flush=True)
                _, body = request('/api/account')
                account = json.loads(body)
                assert 'edition' not in account and 'permissions' in account
                print('PASS: account API returns permissions and locale without an edition', flush=True)
                _, body = request('/api/schema')
                schema = json.loads(body)
                assert set(schema['fields']['x:Branding']['properties']) == {'logo'}
                assert 'x:Enterprise' not in schema['objects']
                print('PASS: schema exposes Branding without activation fields', flush=True)
                assert 'pop3' not in json.dumps(schema).lower()
                assert 'pop3' not in json.dumps(account).lower()
                print('PASS: schema and account permissions expose no POP3 protocol or permissions', flush=True)
                for endpoint in ['/api/token/tracing', '/api/token/metrics']:
                    status, body = request(endpoint)
                    assert status == 200 and len(body) > 10
                    print('PASS: authorized ' + endpoint + ' works without a license', flush=True)
                try:
                    request('/api/account', False)
                    raise AssertionError('Unauthenticated account request succeeded')
                except urllib.error.HTTPError as error:
                    assert error.code == 401
                print('PASS: authorization remains enforced', flush=True)
                if args.passkeys:
                    subprocess.run(['node', 'webui/tests/passkeys-browser.mjs'], cwd=ROOT,
                                   env={**os.environ, 'STALWART_TEST_BASE_URL': base.replace('127.0.0.1', 'localhost'),
                                        'STALWART_TEST_AUTHORIZATION': authorization}, check=True)
                if args.browser:
                    subprocess.run(['node', 'webui/tests/server-browser-smoke.mjs'], cwd=ROOT,
                                   env={**os.environ, 'STALWART_TEST_BASE_URL': base,
                                        'STALWART_TEST_AUTHORIZATION': authorization}, check=True)
            except Exception:
                log.flush()
                print((work / 'server.log').read_text())
                raise
            finally:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()


if __name__ == '__main__':
    main()
