# SPDX-License-Identifier: AGPL-3.0-only
"""Guard the open-source distribution contract against dependency/CI regressions."""
import gzip
import json
from pathlib import Path
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[3]


class DistributionPolicyTests(unittest.TestCase):
    def test_no_workspace_package_can_reenable_an_enterprise_build(self):
        for manifest in (ROOT / 'crates').glob('*/Cargo.toml'):
            package = tomllib.loads(manifest.read_text())
            self.assertNotIn('enterprise', package.get('features', {}), str(manifest))
        self.assertFalse((ROOT / 'crates/common/src/enterprise').exists())

    def test_shipped_schema_has_branding_without_activation(self):
        with gzip.open(ROOT / 'resources/schema/schema.json.gz') as stream:
            schema = json.load(stream)
        self.assertEqual(set(schema['fields']['x:Branding']['properties']), {'logo'})
        self.assertNotIn('x:Enterprise', schema['objects'])
        self.assertNotIn('licenseKey', json.dumps(schema))

    def test_installer_does_not_replace_the_fork_with_an_upstream_binary(self):
        installer = (ROOT / 'install.sh').read_text()
        self.assertIn('readonly BASE_URL="https://github.com/imabee-glacier/stalwart/releases/latest/download"', installer)
        self.assertNotIn('https://github.com/stalwartlabs/stalwart/releases', installer)
