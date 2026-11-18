#!/usr/bin/env python3
"""Put the licence notices inside a content `.hmod`, so a downloaded copy carries them.

A `.hmod` is the deliverable of a content port; the repository's `LICENSE` (and
`LICENSE-NOTICE.md`/`COPYING`) does not travel with it by itself. The content
loader reads only `horizon-mod.toml`, `assets/` and `data/`
(`horizon_mods::Content::read`), so a notice file at the archive root is shipped
and never interpreted.

The rewritten archive is deterministic: every entry gets the same fixed date and
mode, entries keep the build's order, and the notices are appended in a fixed
order. Two runs over the same build give the same bytes and the same SHA-256.

    python3 tools/notice.py content/<id>.hmod
"""
import hashlib
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NOTICES = ('LICENSE', 'LICENCE', 'LICENSE-MIT', 'LICENSE-CC-BY-4.0', 'NOTICE',
           'LICENSE-NOTICE.md', 'COPYING', 'COPYING.LESSER')
DATE = (2026, 1, 1, 0, 0, 0)


def inject(path: Path) -> dict:
    with zipfile.ZipFile(path) as archive:
        entries = [(info.filename, archive.read(info.filename)) for info in archive.infolist()]
    names = {name for name, _ in entries}
    added = []
    for notice in NOTICES:
        source = ROOT / notice
        if source.is_file() and notice not in names:
            entries.append((notice, source.read_bytes()))
            added.append(notice)
    with zipfile.ZipFile(path, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        for name, content in entries:
            info = zipfile.ZipInfo(name, DATE)
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            archive.writestr(info, content)
    return dict(file=str(path), size=path.stat().st_size, added=added,
                entries=len(entries),
                sha256=hashlib.sha256(path.read_bytes()).hexdigest())


if __name__ == '__main__':
    if len(sys.argv) != 2:
        raise SystemExit('usage: tools/notice.py <mod>.hmod')
    result = inject(Path(sys.argv[1]))
    print(f"{result['file']} size={result['size']} sha256={result['sha256']} "
          f"entries={result['entries']} added={','.join(result['added']) or 'none'}")
