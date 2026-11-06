"""Read one exact version section from the repository's CHANGELOG format."""
import re


def release_notes(changelog, version):
    lines = changelog.splitlines()
    heading = re.compile(rf'##[ \t]+{re.escape(version)}[ \t]*')
    starts = [index for index, line in enumerate(lines) if heading.fullmatch(line)]
    if len(starts) != 1:
        raise ValueError(f'Write exactly one ## {version} section in CHANGELOG.md before releasing')
    start = starts[0] + 1
    end = next((index for index in range(start, len(lines))
                if re.match(r'##[ \t]+', lines[index])), len(lines))
    notes = '\n'.join(lines[start:end]).strip()
    if not notes:
        raise ValueError(f'Write release notes under ## {version} in CHANGELOG.md before releasing')
    return notes
