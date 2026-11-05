#!/usr/bin/env python3
"""Build deterministic, compressed login assets from the checked-out source.

No source is downloaded or replaced. --check detects stale generated assets.
"""
# SPDX-License-Identifier: AGPL-3.0-only
import argparse
import gzip
import hashlib
import io
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2] / 'resources/html-templates'

def compress(data):
    # GzipFile fixes filename, mtime and OS metadata across Python 3.11–3.14.
    output = io.BytesIO()
    with gzip.GzipFile(fileobj=output, mode='wb', filename='', mtime=0, compresslevel=9) as stream:
        stream.write(data)
    return output.getvalue()


def build(check=False):
    css, js = (ROOT / 'ui.css').read_bytes(), (ROOT / 'ui.js').read_bytes()
    version = hashlib.sha256(css + b'\0' + js).hexdigest()[:12]
    outputs = {'ui.hash': version.encode(), 'ui.css.min.gz': compress(css), 'ui.js.min.gz': compress(js)}
    for name in ('login', 'calendar-rsvp'):
        html = (ROOT / f'{name}.html').read_bytes().replace(b'ui-HASH', ('ui-' + version).encode())
        outputs[f'{name}.html.min'] = html
        outputs[f'{name}.html.min.gz'] = compress(html)
    for name, data in outputs.items():
        path = ROOT / name
        if check:
            if path.read_bytes() != data:
                raise SystemExit(f'{name} is stale. Run python3 resources/scripts/build_login_assets.py')
        else:
            path.write_bytes(data)

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    build(parser.parse_args().check)
