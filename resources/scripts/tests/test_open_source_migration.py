# SPDX-License-Identifier: AGPL-3.0-only
import importlib.util
from pathlib import Path
import sys
import unittest

path = Path(__file__).resolve().parents[1] / 'migrate_v016.py'
spec = importlib.util.spec_from_file_location('migrate_v016', path)
migration = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = migration
spec.loader.exec_module(migration)

class BrandingMigrationTests(unittest.TestCase):
    def test_activation_credentials_are_not_imported(self):
        converter = migration.Converter([], {
            'enterprise.license-key': 'expired-license',
            'enterprise.api-key': 'unused-renewal-secret',
        })
        self.assertIsNone(converter._build_branding())

    def test_branding_is_imported_without_activation(self):
        converter = migration.Converter([], {
            'enterprise.license-key': 'expired-license',
            'enterprise.api-key': 'unused-renewal-secret',
            'enterprise.logo-url': 'https://example.org/logo.png',
        })
        self.assertEqual(converter._build_branding(), {'logo': 'https://example.org/logo.png'})
        self.assertIn('Branding', migration.SINGLETON_ORDER)
        self.assertNotIn('Enterprise', migration.SINGLETON_ORDER)

if __name__ == '__main__':
    unittest.main()
