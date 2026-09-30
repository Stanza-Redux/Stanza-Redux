# SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
"""Fail before dayscript if its owned OPDS fixture cannot start."""
import os
import time
from urllib.request import ProxyHandler, build_opener


def wait_ready(timeout=30):
    # This is an owned loopback service, never an HTTP proxy or a system PAC destination.
    opener = build_opener(ProxyHandler({}))
    address = 'http://127.0.0.1:' + os.environ.get('STANZA_FIXTURE_PORT', '18765') + '/health'
    expected = os.environ['STANZA_FIXTURE_TOKEN'].encode()
    deadline = time.monotonic() + timeout
    last_error = 'no response'
    while time.monotonic() < deadline:
        try:
            with opener.open(address, timeout=1) as response:
                if response.read() == expected:
                    return
                last_error = 'port is occupied by a different fixture (ownership token differs)'
        except OSError as error:
            last_error = str(error)
        time.sleep(.1)
    raise RuntimeError(f'OPDS fixture at {address} did not become ready: {last_error}')


if __name__ == '__main__':
    wait_ready()
