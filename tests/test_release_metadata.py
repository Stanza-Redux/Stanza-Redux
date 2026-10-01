# SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
"""Release contracts: no untranslated UI gaps, stale store identity, or broken book fixture."""
from pathlib import Path
import hashlib
import json
import re
import unittest
from showcase_catalog import BOOKS, publication, response

ROOT = Path(__file__).resolve().parents[1]
LOCALES = {'en', 'ar', 'de', 'es', 'fr', 'hi', 'id', 'it', 'ja', 'ko', 'pt-BR', 'ru', 'zh-CN'}


def catalog(path):
    return dict(line.split(' = ', 1) for line in path.read_text().splitlines() if ' = ' in line)


class ReleaseTests(unittest.TestCase):
    def test_v1_languages_and_interpolation_arguments_are_complete(self):
        paths = list((ROOT / 'resource/locales').glob('*/app.ftl'))
        self.assertEqual({p.parent.name for p in paths}, LOCALES)
        english = catalog(ROOT / 'resource/locales/en/app.ftl')
        for path in paths:
            translated = catalog(path)
            self.assertEqual(translated.keys(), english.keys(), path.parent.name)
            for key, value in translated.items():
                with self.subTest(locale=path.parent.name, key=key):
                    self.assertTrue(value.strip())
                    self.assertEqual(set(re.findall(r'\$([a-z_]+)', value)),
                                     set(re.findall(r'\$([a-z_]+)', english[key])))

    def test_screenshot_epubs_match_their_recorded_sources(self):
        for entry in json.loads((ROOT / 'tests/fixtures/books/provenance.json').read_text()):
            book = ROOT / 'tests/fixtures/books' / entry['file']
            self.assertEqual(hashlib.sha256(book.read_bytes()).hexdigest(), entry['sha256'])

    def test_screenshot_fixtures_stay_small_and_match_walkthrough_cover_ids(self):
        fixtures = ROOT / 'tests/fixtures/books'
        recorded = json.loads((fixtures / 'provenance.json').read_text())
        self.assertEqual({p.name for p in fixtures.glob('*.epub')},
                         {entry['file'] for entry in recorded})
        walkthrough = (ROOT / 'dayscript/store-walkthrough.yaml').read_text()
        for path in fixtures.glob('*.epub'):
            self.assertLess(path.stat().st_size, 1024 * 1024, path.name)
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            self.assertIn('library:books/' + digest + '.cover', walkthrough)
        for key in BOOKS:
            self.assertIn('/showcase/cover/' + key, walkthrough)

    def test_catalog_acquisitions_covers_and_metadata_are_real(self):
        seen = set()
        for key in BOOKS:
            entry = publication(key)
            self.assertTrue(entry['metadata']['title'])
            self.assertTrue(entry['metadata']['author'])
            self.assertTrue(entry['metadata']['subject'])
            self.assertNotIn(entry['metadata']['identifier'], seen)
            seen.add(entry['metadata']['identifier'])
            data, mime = response(entry['links'][0]['href'])
            self.assertEqual(mime, 'application/epub+zip')
            self.assertEqual(data, BOOKS[key].read_bytes())
            cover, mime = response(entry['images'][0]['href'])
            self.assertTrue(mime.startswith('image/'))
            self.assertGreater(len(cover), 1000)
        self.assertIsNone(response('/showcase/book/../../secret'))

    def test_store_metadata_has_a_translation_for_every_shipped_locale(self):
        # Test the declaration without adding a TOML parser dependency to Python 3.9 hosts.
        text = (ROOT / 'store/storefront.toml').read_text()
        declared = set(re.findall(r'\[storefront.metadata."([^"]+)"\]', text)) | {'en'}
        self.assertEqual(declared, LOCALES)
        for field in ['subtitle', 'short', 'description', 'keywords', 'release-notes', 'promo']:
            self.assertEqual(len(re.findall('^' + field + ' = ', text, re.M)), len(LOCALES))
