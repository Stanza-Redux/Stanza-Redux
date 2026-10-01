# SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
"""Offline screenshot collection of actual public-domain publications, not app UI copy."""
from pathlib import Path
import json
import posixpath
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parents[1]
BOOKS = {
    'alice': ROOT / 'resource/assets/Alice.epub',
    'frankenstein': ROOT / 'tests/fixtures/books/frankenstein.epub',
    'the-time-machine': ROOT / 'tests/fixtures/books/the-time-machine.epub',
}
DC = '{http://purl.org/dc/elements/1.1/}'
OPF = '{http://www.idpf.org/2007/opf}'
DESCRIPTIONS = {
    'alice': 'Alice follows a white rabbit into a world of curious creatures, riddles, and unexpected adventures. Illustrated by John Tenniel.',
    'frankenstein': 'Victor Frankenstein brings a creature to life, then abandons it. Mary Shelley’s novel explores ambition, responsibility, and the need for companionship.',
    'the-time-machine': 'A Victorian inventor travels into the distant future, encountering the Eloi and Morlocks as he explores what has become of humanity.',
}

def publication(key):
    with zipfile.ZipFile(BOOKS[key]) as archive:
        path = next(n for n in archive.namelist() if n.endswith('.opf'))
        package = ET.fromstring(archive.read(path))
        metadata = package.find(OPF + 'metadata')
        text = lambda name: metadata.findtext(DC + name, '')
        return {'metadata': {
            'title': text('title'), 'author': text('creator'),
            'identifier': text('identifier'), 'language': [text('language')],
            'subject': [n.text for n in metadata.findall(DC + 'subject')],
            'rights': text('rights'), 'publisher': 'Project Gutenberg',
            'description': DESCRIPTIONS[key],
        }, 'images': [{'href': '/showcase/cover/' + key, 'type': 'image/jpeg'}],
        'links': [{'href': '/showcase/book/' + key, 'type': 'application/epub+zip',
                   'rel': 'http://opds-spec.org/acquisition/open-access'}]}

def response(path):
    if path == '/showcase':
        return json.dumps({'metadata': {'title': 'Public-domain classics'},
                           'publications': [publication(k) for k in BOOKS]}).encode(), 'application/opds+json'
    for kind in ['book', 'cover']:
        prefix = '/showcase/' + kind + '/'
        if path.startswith(prefix) and path[len(prefix):] in BOOKS:
            book = BOOKS[path[len(prefix):]]
            if kind == 'book': return book.read_bytes(), 'application/epub+zip'
            with zipfile.ZipFile(book) as archive:
                path = next(n for n in archive.namelist() if n.endswith('.opf'))
                package = ET.fromstring(archive.read(path))
                cover = next(i for i in package.find(OPF + 'manifest') if 'cover-image' in i.get('properties', '').split())
                member = posixpath.normpath(posixpath.join(posixpath.dirname(path), cover.get('href')))
                return archive.read(member), cover.get('media-type')
    return None
