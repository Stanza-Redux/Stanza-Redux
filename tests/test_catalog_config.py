# SPDX-License-Identifier: GPL-2.0-or-later
"""The website's compatibility endpoints and offline bundle have one source of truth."""
import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('catalog_config_generator', ROOT / 'scripts/sync_catalog_config.py')
generator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(generator)


class CatalogConfigurationTests(unittest.TestCase):
    def test_generated_outputs_are_current(self):
        for name, expected in generator.outputs().items():
            with self.subTest(path=name):
                self.assertEqual((ROOT / name).read_bytes(), expected,
                                 'Run python3 scripts/sync_catalog_config.py')

    def test_legacy_catalogs_exclude_disabled_entries(self):
        outputs = generator.outputs()
        legacy = json.loads(outputs['website/public/catalogs.json'])
        self.assertTrue(legacy['catalogs'])
        config = json.loads(outputs['resource/assets/catalog-config.json'])
        self.assertEqual(json.loads(outputs['website/public/censorship.json']), config['censorship'])
        gutenberg = [c for c in config['catalogs'] if 'gutenberg.org' in c['url']]
        self.assertEqual(len(gutenberg), 2)
        active = {c['id'] for c in config['catalogs'] if c.get('enabled', True)}
        self.assertEqual({c['id'] for c in legacy['catalogs']}, active)


if __name__ == '__main__':
    unittest.main()
