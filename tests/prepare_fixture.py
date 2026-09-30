# SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
"""Fail before dayscript if its owned OPDS fixture cannot start."""
import os
import time
from urllib.request import urlopen


def wait_ready(timeout=10):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            with urlopen('http://127.0.0.1:' + os.environ.get('STANZA_FIXTURE_PORT', '18765') + '/health', timeout=1) as response:
                if response.read() == os.environ['STANZA_FIXTURE_TOKEN'].encode():
                    return
        except OSError:
            pass
        time.sleep(.1)
    raise RuntimeError('OPDS fixture did not become ready; inspect stanza-catalog-fixture.log')


if __name__ == '__main__':
    wait_ready()
