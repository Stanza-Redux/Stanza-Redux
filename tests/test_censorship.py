# SPDX-License-Identifier: GPL-2.0-or-later
"""Deployment contracts for the combined configuration and legacy policy endpoint."""
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class CensorshipTests(unittest.TestCase):
    def test_published_registry_is_in_the_sites_public_directory(self):
        self.assertTrue((ROOT / 'website/public/censorship.json').is_file())
        self.assertIn("publicDirs: ['./public']", (ROOT / 'website/daysite.config.mjs').read_text())

    def test_messages_cover_every_shipped_locale(self):
        locales = {p.parent.name for p in (ROOT / 'resource/locales').glob('*/app.ftl')}
        for source in ['website/public/censorship.json', 'website/public/catalog-config.json', 'resource/assets/catalog-config.json']:
            with self.subTest(source=source):
                data = json.loads((ROOT / source).read_text())
                data = data.get('censorship', data)
                self.assertEqual(data['version'], 1)
                for message in data['messages'].values():
                    self.assertEqual(set(message), locales)
                    self.assertTrue(all(text.strip() for text in message.values()))
                for rule in data['rules']:
                    if rule.get('cover_message'):
                        self.assertIn(rule['cover_message'], data['messages'])
