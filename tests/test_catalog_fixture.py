# SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
"""Exercise the HTTP fixture without using a developer's fixed port."""
import os
import threading
import unittest
from http.server import ThreadingHTTPServer
from urllib.request import urlopen
from unittest.mock import patch
from catalog_server import Handler


class CatalogFixtureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()
        cls.base = 'http://127.0.0.1:' + str(cls.server.server_port)

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join()

    def test_health_identifies_the_owned_fixture(self):
        with patch.dict(os.environ, STANZA_FIXTURE_TOKEN='owned-fixture-123'):
            with urlopen(self.base + '/health') as response:
                self.assertEqual(response.read(), b'owned-fixture-123')

    def test_cancellation_streams_allow_browser_cors(self):
        for path in ['/cancel-feed', '/cancel-cover']:
            with urlopen(self.base + path) as response:
                self.assertEqual(response.headers['Access-Control-Allow-Origin'], '*')
                self.assertEqual(response.headers['Content-Length'], '1048576')
                self.assertEqual(len(response.read(1)), 1)

    def test_search_template_uses_the_listening_port(self):
        with urlopen(self.base + '/search.xml') as response:
            body = response.read().decode()
            self.assertIn(self.base + '/results?q={searchTerms}', body)

    def test_feed_and_epub_are_available(self):
        with urlopen(self.base + '/rich') as response:
            self.assertEqual(response.status, 200)
            self.assertIn(b'Book collection', response.read())
        with urlopen(self.base + '/alice.epub') as response:
            self.assertEqual(response.headers['Content-Type'], 'application/epub+zip')
            self.assertEqual(response.read(2), b'PK')


if __name__ == '__main__':
    unittest.main()
