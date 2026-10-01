#!/usr/bin/env python3
"""Deterministic OPDS/EPUB fixture. Run before dayscript/catalog.yaml (port 18765)."""
# Start diagnostics before potentially slow standard-library imports. This process is an
# owned CI fixture; a blocked startup/accept must leave a stack in its captured log.
if __name__ == '__main__':
    import faulthandler
    faulthandler.enable()
    faulthandler.dump_traceback_later(10, repeat=True)
    print('OPDS fixture: importing server modules', flush=True)

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from socketserver import TCPServer
from pathlib import Path
import json
from showcase_catalog import response as showcase_response
import os
import time
import struct
import zlib
import zipfile
import threading
from urllib.parse import urlparse
class FixtureServer(ThreadingHTTPServer):
    def server_bind(self):
        # HTTPServer resolves getfqdn(host) here; our numeric loopback fixture needs no
        # DNS, and runner resolver configuration must not hold up socket activation.
        TCPServer.server_bind(self)
        self.server_name, self.server_port = self.server_address[:2]

BOOK = Path(__file__).resolve().parents[1] / 'resource/assets/Alice.epub'
# Unique, high-contrast covers make reuse mistakes visible without external services.
COVER_COLORS = [(194, 61, 65), (47, 117, 181), (43, 143, 102), (135, 79, 171), (210, 139, 44)]
def test_cover(index):
    color = bytes(COVER_COLORS[index % len(COVER_COLORS)])
    width, height = 92, 128
    rows = []
    for y in range(height):
        row = bytearray()
        for x in range(width):
            # Book spine, title bars and row-number ticks, authored directly as pixels.
            paper = 20 < x < 77 and (24 < y < 29 or 36 < y < 40)
            tick = 20 < x < 20 + (index % 10 + 1) * 5 and 99 < y < 106
            row.extend(b'\xf4\xf0\xe5' if paper or tick else color)
        rows.append(b'\0' + row)
    def chunk(tag, data):
        return struct.pack('>I', len(data)) + tag + data + struct.pack('>I', zlib.crc32(tag + data))
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(b''.join(rows))) + chunk(b'IEND', b'')
CANCELLED = {'feed': 0, 'cover': 0}
COUNTS_LOCK = threading.Lock()
class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if __name__ == '__main__':
            faulthandler.cancel_dump_traceback_later()
        path = urlparse(self.path).path
        self.log_message('User-Agent: %s', self.headers.get('User-Agent', ''))
        status, mime = 200, 'application/atom+xml'
        if path in ('/cancel-feed', '/cancel-cover'):
            kind = 'feed' if path == '/cancel-feed' else 'cover'
            # Send a response body slowly so cancellation is observable as a disconnected
            # socket, including after headers have already arrived.
            self.send_response(200)
            self.send_header('Content-Type', 'application/opds+json' if kind == 'feed' else 'image/png')
            self.send_header('Content-Length', '1048576')
            self.send_header('Access-Control-Allow-Origin', '*')
            self.end_headers()
            try:
                for _ in range(256):
                    self.wfile.write(b' ' * 4096); self.wfile.flush(); time.sleep(.05)
            except (BrokenPipeError, ConnectionResetError):
                with COUNTS_LOCK: CANCELLED[kind] += 1
                self.log_message('cancelled %s request', kind)
            return
        showcase = showcase_response(path) if path.startswith('/showcase') else None
        if showcase is not None:
            data, mime = showcase
        elif path == '/health':
            data, mime = os.environ.get('STANZA_FIXTURE_TOKEN', 'stanza-catalog-fixture').encode(), 'text/plain'
        elif path == '/loading-fixture':
            mime = 'application/opds+json'
            data = json.dumps({'metadata': {'title': 'Loading fixture'}, 'navigation': [
                {'title': 'Slow catalog', 'href': '/slow'},
                {'title': 'Missing catalog', 'href': '/missing'},
                {'title': 'Atom catalog', 'href': '/catalog'},
            ]}).encode()
        elif path == '/cancellation':
            with COUNTS_LOCK: CANCELLED.update(feed=0, cover=0)
            mime = 'application/opds+json'
            data = json.dumps({'metadata': {'title': 'Cancellation fixture'}, 'navigation': [
                {'title': 'Slow destination', 'href': '/cancel-feed'},
                {'title': 'Slow thumbnail list', 'href': '/cancel-covers'},
                {'title': 'Cancellation results', 'href': '/cancel-results'},
            ]}).encode()
        elif path == '/cancel-covers':
            mime = 'application/opds+json'
            data = json.dumps({'metadata': {'title': 'Slow thumbnails'}, 'publications': [
                {'metadata': {'title': 'Pending cover'}, 'images': [{'href': '/cancel-cover'}], 'links': []}
            ]}).encode()
        elif path == '/cancel-results':
            # Allow network cancellation to reach the fixture; zero never counts as success.
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline and not all(CANCELLED.values()): time.sleep(.05)
            mime = 'application/opds+json'
            title = 'Requests cancelled' if all(CANCELLED.values()) else 'Cancellation failed'
            data = json.dumps({'metadata': {'title': title}, 'navigation': []}).encode()
        elif path == '/many':
            mime = 'application/opds+json'
            data = json.dumps({'metadata': {'title': 'Thumbnail stress'}, 'publications': [
                {'metadata': {'title': f'Book {i:03d}', 'author': f'Author {i:03d}'},
                 'images': [{'href': f'/covers/{i}.png'}],
                 'links': [{'href': '/alice.epub', 'type': 'application/epub+zip'}]}
                for i in range(120)
            ]}).encode()
        elif path.startswith('/covers/'):
            index = int(path.rsplit('/', 1)[1].split('.')[0])
            # Overlapping requests finish in a different order while the list scrolls.
            time.sleep(2.0 if index == 0 else 0.3 + (index % 4) * 0.35)
            data, mime = test_cover(index), 'image/png'
        elif path in ('/cover.jpg', '/slow-cover.jpg'):
            if path == '/slow-cover.jpg': time.sleep(2)
            with zipfile.ZipFile(BOOK) as epub:
                data = epub.read(next(n for n in epub.namelist() if n.endswith('_cover.jpg')))
            mime = 'image/jpeg'
        elif path == '/bad-cover.jpg':
            data, mime = b'not an image', 'image/jpeg'
        elif path == '/slow':
            time.sleep(3)
            mime = 'application/opds+json'
            data = json.dumps({'metadata': {'title': 'Slow catalog'}, 'publications': [
                {'metadata': {'title': title}, 'images': [{'href': cover}], 'links': [{'href': '/alice.epub', 'type': 'application/epub+zip'}]}
                for title, cover in [('Delayed cover', '/slow-cover.jpg'), ('Broken cover', '/bad-cover.jpg'), ('Missing cover', '/missing-cover.jpg')]
            ]}).encode()
        elif path == '/alice.epub':
            data, mime = BOOK.read_bytes(), 'application/epub+zip'
        elif path == '/search.xml':
            data = b'<OpenSearchDescription xmlns="http://a9.com/-/spec/opensearch/1.1/"><Url type="application/atom+xml" template="http://127.0.0.1:PORT/results?q={searchTerms}"/></OpenSearchDescription>'
        elif path in ('/subjects', '/authors', '/facet'):
            mime = 'application/opds+json'
            next_path = {'/subjects':'/authors','/authors':'/facet','/facet':'/json'}[path]
            data = json.dumps({'metadata':{'title':path[1:].title()},'navigation':[{'title':'Continue browsing','href':next_path,'type':'application/opds+json'}], 'facets':[{'metadata':{'title':'Language'},'links':[{'title':'English','href':'/facet','type':'application/opds+json'}]}]}).encode()
        elif path == '/rich':
            mime = 'application/opds+json'
            data = json.dumps({'metadata': {'title': 'Book collection'}, 'publications': [{'metadata': {
                'title': 'Alice in Wonderland: Adventures through a very curious looking glass and beyond',
                'author': 'Lewis Carroll', 'subject': [{'name': 'Fantasy'}, {'name': 'Classics'}],
                'publisher': 'Project Gutenberg', 'published': '1865', 'modified': '2024-09-01',
                'language': ['en'], 'identifier': 'gutenberg:11', 'numberOfPages': 123,
                'description': '<p>A journey into a world of curious creatures and unexpected discoveries.</p>',
                'rights': 'Public domain in the USA', 'illustrator': {'name': 'John Tenniel'}},
                'images': [{'href': '/cover.jpg'}], 'links': [{'href': '/alice.epub', 'type': 'application/epub+zip'}]}]}).encode()
        elif path == '/json':
            mime = 'application/opds+json'
            data = json.dumps({'metadata': {'title': 'OPDS 2 fixture'}, 'publications': [{'metadata': {'title': 'Alice in Wonderland', 'author': 'Lewis Carroll'}, 'images': [{'href': '/cover.jpg'}], 'links': [{'href': '/alice.epub', 'type': 'application/epub+zip', 'rel': 'http://opds-spec.org/acquisition/open-access'}]}]}).encode()
        elif path in ('/catalog', '/results', '/page2'):
            data = b'''<feed xmlns="http://www.w3.org/2005/Atom"><title>Stanza test catalog</title><link rel="search" type="application/opensearchdescription+xml" href="/search.xml"/><link rel="next" href="/page2"/><entry><title>Alice in Wonderland</title><author><name>Lewis Carroll</name></author><summary>A local acquisition fixture.</summary><link rel="http://opds-spec.org/image/thumbnail" href="/cover.jpg"/><link rel="http://opds-spec.org/acquisition/open-access" type="application/epub+zip" href="/alice.epub"/></entry></feed>'''
            if path == '/results': data = data.replace(b'Stanza test catalog', b'Search results')
        else:
            status, data = 404, b'Not found'
        if path == '/search.xml':
            data = data.replace(b':PORT/', (':' + str(self.server.server_port) + '/').encode())
        self.send_response(status)
        self.send_header('Content-Type', mime)
        self.send_header('Content-Length', str(len(data)))
        self.send_header('Access-Control-Allow-Origin', '*')
        self.end_headers()
        try: self.wfile.write(data)
        except (BrokenPipeError, ConnectionResetError): pass
if __name__ == '__main__':
    address = ('127.0.0.1', int(os.environ.get('STANZA_FIXTURE_PORT', '18765')))
    print(f'OPDS fixture: binding {address}, pid={os.getpid()}', flush=True)
    with FixtureServer(address, Handler) as server:
        print(f'OPDS fixture: listening on {server.server_address}', flush=True)
        server.serve_forever()
