#!/usr/bin/env python3
"""Fail closed when the reviewed RSA advisory applicability changes."""
# SPDX-License-Identifier: AGPL-3.0-only
import datetime
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def validate(metadata, today=None):
    today = today or datetime.date.today()
    if today >= datetime.date(2027, 2, 5):
        raise ValueError('RSA applicability exception expired; repeat the dependency review')
    packages = {p['id']: p for p in metadata['packages']}
    nodes = metadata['resolve']['nodes']
    allowed = {
        '0.9.10': {('common', '1.0.4'), ('mail-auth', '0.15.1')},
        '0.10.0-rc.19': {('crypto-glue', '0.2.2')},
    }
    for node in nodes:
        package = packages[node['id']]
        name = package['name']
        if name in {'rsa', 'crypto-glue', 'webauthn-rs-core', 'webauthn-attestation-ca', 'mail-auth', 'jsonwebtoken'} and package.get('source') != 'registry+https://github.com/rust-lang/crates.io-index':
            raise ValueError('Reviewed cryptographic source changed; advisory review required')
        features = set(node['features'])
        if name == 'rsa':
            parents = {(packages[p['id']]['name'], packages[p['id']]['version'])
                       for p in nodes if node['id'] in p['dependencies']}
            if package['version'] not in allowed or not parents <= allowed[package['version']]:
                raise ValueError('RSA dependency path changed; advisory review required')
        elif name == 'mail-auth':
            if package['version'] != '0.15.1' or 'aws-lc-rs' not in features or 'rust-crypto' in features:
                raise ValueError('Mail signing backend changed; advisory review required')
        elif name == 'jsonwebtoken':
            if package['version'] != '11.1.0' or 'aws_lc_rs' not in features or 'rust_crypto' in features:
                raise ValueError('JWT signing backend changed; advisory review required')
        elif name in ('webauthn-rs-core', 'webauthn-attestation-ca', 'crypto-glue'):
            expected = '0.2.2' if name == 'crypto-glue' else '0.6.2'
            if package['version'] != expected:
                raise ValueError('WebAuthn crypto implementation changed; advisory review required')
        elif name in ('zenoh', 'zenoh-transport'):
            if features & {'auth_pubkey', 'transport_multilink'}:
                raise ValueError('Vulnerable Zenoh RSA exchange must remain disabled')
        elif name == 'lz4_flex' and package['version'] != '0.14.0':
            raise ValueError('LZ4 decoder changed; advisory review required')


if __name__ == '__main__':
    metadata = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--locked', '--all-features', '--format-version', '1'], cwd=ROOT))
    validate(metadata)
    print('PASS: reviewed RSA applicability and patched LZ4 dependency graph')
