# SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
"""Exercise the HTTP fixture without using a developer's fixed port."""
import os
import socket
import subprocess
import tempfile
from pathlib import Path
import threading
import unittest
from http.server import ThreadingHTTPServer
from urllib.request import urlopen
from unittest.mock import patch
from catalog_server import Handler
from prepare_fixture import wait_ready


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

    def test_readiness_bypasses_system_and_environment_proxies(self):
        with patch.dict(os.environ, STANZA_FIXTURE_TOKEN='probe-token',
                        STANZA_FIXTURE_PORT=str(self.server.server_port)), \
                patch('urllib.request.getproxies', return_value={'http': 'http://127.0.0.1:1'}), \
                patch('urllib.request.proxy_bypass', return_value=False):
            wait_ready(timeout=1)

    def test_readiness_reports_a_foreign_listener(self):
        from io import BytesIO
        from unittest.mock import MagicMock
        opener = MagicMock()
        opener.open.return_value.__enter__.side_effect = lambda: BytesIO(b'foreign')
        with patch.dict(os.environ, STANZA_FIXTURE_TOKEN='owned'), \
                patch('prepare_fixture.build_opener', return_value=opener):
            with self.assertRaisesRegex(RuntimeError, 'ownership token differs'):
                wait_ready(timeout=.15)

    def test_readiness_reports_connection_failure(self):
        from unittest.mock import MagicMock
        opener = MagicMock()
        opener.open.side_effect = OSError('connection refused')
        with patch.dict(os.environ, STANZA_FIXTURE_TOKEN='owned'), \
                patch('prepare_fixture.build_opener', return_value=opener):
            with self.assertRaisesRegex(RuntimeError, 'connection refused'):
                wait_ready(timeout=.15)

    @unittest.skipIf(os.name == 'nt', 'Bash lifecycle is exercised by the Unix CI jobs')
    def test_shell_fixture_cleanup_preserves_failure_and_reports_server_log(self):
        with socket.socket() as reservation:
            reservation.bind(('127.0.0.1', 0))
            port = reservation.getsockname()[1]
        with tempfile.TemporaryDirectory() as directory:
            env = dict(os.environ, STANZA_FIXTURE_PORT=str(port), RUNNER_TEMP=directory,
                       DAY_SCRIPT_TARGET='macos-appkit')
            result = subprocess.run(
                ['/bin/bash', '-c', 'source tests/ci-fixture.sh; exit 17'],
                cwd=Path(__file__).resolve().parents[1], env=env,
                text=True, capture_output=True, timeout=40)
            self.assertEqual(result.returncode, 17, result.stderr)
            self.assertIn('GET /health', result.stderr)
            with socket.socket() as probe:
                probe.settimeout(1)
                self.assertNotEqual(probe.connect_ex(('127.0.0.1', port)), 0)

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
