#!/usr/bin/env python3
"""Package the built dashboard deterministically for embedding in the server."""
# SPDX-License-Identifier: AGPL-3.0-only
import io
from pathlib import Path
import sys
import zipfile

root = Path(__file__).resolve().parent
output = io.BytesIO()
with zipfile.ZipFile(output, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
    for path in sorted((root / 'dist').rglob('*')):
        if path.is_file():
            info = zipfile.ZipInfo(path.relative_to(root / 'dist').as_posix(), date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, path.read_bytes())
data = output.getvalue()
assert zipfile.ZipFile(io.BytesIO(data)).getinfo('index.html')
assert zipfile.ZipFile(io.BytesIO(data)).getinfo('stalwart-app.json')
target = root.parent / 'resources' / 'webui.zip'
if '--check' in sys.argv:
    if target.read_bytes() != data:
        raise SystemExit('Bundled dashboard differs from the current frontend build; run npm run build.')
else:
    target.write_bytes(data)
