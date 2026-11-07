#!/usr/bin/env python3
"""Resolve one freshly built backend harness from Cargo JSON, never a glob."""
import json
from pathlib import Path
import sys


def find_binary(lines, kind, source):
    matches = set()
    for line in lines:
        item = json.loads(line)
        target = item.get('target', {})
        if (item.get('reason') == 'compiler-artifact'
                and item.get('profile', {}).get('test')
                and kind in target.get('kind', [])
                and Path(target.get('src_path', '')).resolve() == source.resolve()
                and item.get('executable')):
            matches.add(Path(item['executable']).resolve(strict=True))
    if len(matches) != 1:
        raise ValueError(f'Expected one {kind} test harness for {source}, found {len(matches)}')
    result = matches.pop()
    if not result.is_file():
        raise ValueError(f'Not a regular executable: {result}')
    return result


if __name__ == '__main__':
    log, kind, source = sys.argv[1:]
    with open(log) as stream:
        print(find_binary(stream, kind, Path(source)))
