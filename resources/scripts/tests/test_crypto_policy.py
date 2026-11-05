# SPDX-License-Identifier: AGPL-3.0-only
import datetime
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    'crypto_policy', Path(__file__).resolve().parents[1] / 'check_crypto_policy.py')
policy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(policy)


class CryptoPolicyTests(unittest.TestCase):
    def metadata(self, parent='common', version='0.9.10'):
        return {
            'packages': [
                {'id': 'rsa', 'name': 'rsa', 'version': version,
                 'source': 'registry+https://github.com/rust-lang/crates.io-index'},
                {'id': 'parent', 'name': parent, 'version': '1.0.3'},
            ],
            'resolve': {'nodes': [
                {'id': 'rsa', 'features': [], 'dependencies': []},
                {'id': 'parent', 'features': [], 'dependencies': ['rsa']},
            ]},
        }

    def check(self, metadata):
        policy.validate(metadata, datetime.date(2026, 11, 5))

    def test_only_reviewed_parent_and_version_are_accepted(self):
        self.check(self.metadata())
        for metadata in [self.metadata('new-crypto-client'), self.metadata(version='0.9.11')]:
            with self.assertRaises(ValueError):
                self.check(metadata)

    def test_changed_source_and_expired_review_fail_closed(self):
        metadata = self.metadata()
        metadata['packages'][0]['source'] = None
        with self.assertRaises(ValueError):
            self.check(metadata)
        with self.assertRaises(ValueError):
            policy.validate(self.metadata(), datetime.date(2027, 2, 5))

    def test_legacy_network_rsa_features_are_rejected(self):
        for feature in ['auth_pubkey', 'transport_multilink']:
            metadata = self.metadata()
            metadata['packages'].append({'id': 'zenoh', 'name': 'zenoh-transport', 'version': '1.10.2'})
            metadata['resolve']['nodes'].append({'id': 'zenoh', 'features': [feature], 'dependencies': []})
            with self.assertRaises(ValueError):
                self.check(metadata)
