# SPDX-License-Identifier: AGPL-3.0-only
"""Guard the open-source distribution contract against dependency/CI regressions."""
import gzip
import json
import re
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

    def test_installer_verifies_release_checksums(self):
        installer = (ROOT / 'install.sh').read_text()
        self.assertIn('verify_sha256 "$_tar" "${_tar}.sha256"', installer)
        download = installer.index('ensure downloader "${BASE_URL}/${_component}-${_arch}.tar.gz" "$_tar"')
        extract = installer.index('ensure tar zxf "$_tar"')
        verify = installer.index('verify_sha256 "$_tar"')
        self.assertLess(download, verify)
        self.assertLess(verify, extract)
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        self.assertIn('sha256sum "$(basename "$file")" > "$(basename "$file").sha256"', workflow)
        self.assertIn('archive/**/*.sha256', workflow)

    def test_release_builds_never_enable_test_hooks(self):
        # `stalwart_tests` compiles in test-only hooks; it must never reach a shipped binary.
        candidates = [
            ROOT / '.cargo/config.toml',
            ROOT / 'docker-bake.hcl',
            *ROOT.glob('Dockerfile*'),
            *(ROOT / '.github/workflows').glob('*.yml'),
        ]
        for path in candidates:
            if path.exists():
                self.assertNotIn('stalwart_tests', path.read_text(), str(path))

    def test_workflow_actions_are_pinned_to_commits(self):
        pattern = re.compile(r'^\s*(?:-\s*)?uses:\s*(\S+)', re.M)
        for workflow in (ROOT / '.github/workflows').glob('*.yml'):
            for action in pattern.findall(workflow.read_text()):
                if action.startswith('./'):
                    continue
                self.assertRegex(action, r'@[0-9a-f]{40}$', f'{workflow.name}: {action}')
