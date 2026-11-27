#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Generate bundled configuration and compatibility endpoints from one editable document."""
import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def outputs(root=ROOT):
    source = (root / 'website/public/catalog-config.json').read_bytes()
    config = json.loads(source)
    if config['version'] != 1:
        raise ValueError('unsupported catalog configuration version')
    catalogs = {'version': 1, 'catalogs': [
        {k: v for k, v in entry.items() if k != 'enabled'}
        for entry in config['catalogs'] if entry.get('enabled', True)
    ]}
    encoded = lambda value: (json.dumps(value, ensure_ascii=False, indent=2) + '\n').encode()
    return {
        'resource/assets/catalog-config.json': source,
        # Released apps still request these URLs. Do not edit them by hand.
        'website/public/catalogs.json': encoded(catalogs),
        'website/public/censorship.json': encoded(config['censorship']),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    stale = []
    for name, content in outputs().items():
        path = ROOT / name
        if not path.exists() or path.read_bytes() != content:
            if args.check:
                stale.append(name)
            else:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(content)
    if stale:
        parser.exit(1, 'Run python3 scripts/sync_catalog_config.py; stale: ' + ', '.join(stale) + '\n')


if __name__ == '__main__':
    main()
