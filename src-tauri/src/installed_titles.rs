//! Optional, bounded local display metadata. Never changes game identity or data.
use super::*;
use std::io::Read;

const MAX_SFO_BYTES: usize = 64 * 1024;
const MAX_SFO_ENTRIES: usize = 1024;
const MAX_TITLE_BYTES: usize = 1024;

pub(crate) fn title(id: &str, game: &Path, roots: &[PathBuf]) -> Option<String> {
    let (directory, metadata) = if is_ps3_eboot(id, game) {
        let directory = game.parent()?.parent()?;
        (directory, directory.join("PARAM.SFO"))
    } else if is_ps4_eboot(id, game) {
        let directory = game.parent()?;
        (directory, directory.join("sce_sys/param.sfo"))
    } else {
        return None;
    };
    // Even registering just USRDIR must not implicitly authorize sibling reads.
    // Do not follow a metadata file symlink or accept devices/directories/FIFOs.
    if !fs::symlink_metadata(&metadata).ok()?.file_type().is_file() {
        return None;
    }
    let path = metadata.canonicalize().ok()?;
    if !path.starts_with(directory) || libraries::containing_root(&path, roots).is_none() {
        return None;
    }
    let file = fs::File::open(&path).ok()?;
    let stat = file.metadata().ok()?;
    if !stat.is_file() || stat.len() > MAX_SFO_BYTES as u64 {
        return None;
    }
    let mut bytes = Vec::new();
    file.take((MAX_SFO_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    sfo_title(&bytes)
}

fn u16_at(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn u32_at(bytes: &[u8], offset: usize) -> Option<usize> {
    Some(u32::from_le_bytes(bytes.get(offset..offset + 4)?.try_into().ok()?) as usize)
}

fn sfo_title(bytes: &[u8]) -> Option<String> {
    if bytes.len() > MAX_SFO_BYTES
        || bytes.get(..4)? != b"\0PSF"
        || !matches!(u32_at(bytes, 4)?, 0x100 | 0x101)
    {
        return None;
    }
    let keys = u32_at(bytes, 8)?;
    let data = u32_at(bytes, 12)?;
    let count = u32_at(bytes, 16)?;
    if count > MAX_SFO_ENTRIES || 20 + count * 16 > keys || keys > data || data > bytes.len() {
        return None;
    }
    let mut result = None;
    let mut names = HashSet::new();
    for index in 0..count {
        let entry = 20 + index * 16;
        let key_start = keys.checked_add(u16_at(bytes, entry)? as usize)?;
        let key_tail = bytes.get(key_start..data)?;
        let key = key_tail.get(..key_tail.iter().position(|b| *b == 0)?)?;
        if !names.insert(key) {
            return None;
        }
        let length = u32_at(bytes, entry + 4)?;
        let capacity = u32_at(bytes, entry + 8)?;
        let start = data.checked_add(u32_at(bytes, entry + 12)?)?;
        if length > capacity || start.checked_add(capacity)? > bytes.len() {
            return None;
        }
        if key != b"TITLE" {
            continue;
        }
        if u16_at(bytes, entry + 2)? != 0x0204 || length == 0 || length > MAX_TITLE_BYTES + 1 {
            return None;
        }
        let value = bytes.get(start..start.checked_add(length)?)?;
        let end = value.iter().position(|b| *b == 0)?;
        if value[end..].iter().any(|b| *b != 0) {
            return None;
        }
        let text = std::str::from_utf8(&value[..end]).ok()?;
        if text.chars().any(|c| c.is_control() && !c.is_whitespace()) {
            return None;
        }
        // Keep the real title's punctuation and brackets, unlike filename cleanup.
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() {
            return None;
        }
        result = Some(text);
    }
    result
}

#[cfg(test)]
pub(crate) fn fixture(title: &str) -> Vec<u8> {
    let length = (title.len() + 1) as u32;
    let mut bytes = Vec::new();
    bytes.extend(b"\0PSF");
    for value in [0x101u32, 36, 44, 1] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(0u16.to_le_bytes());
    bytes.extend(0x0204u16.to_le_bytes());
    for value in [length, length, 0] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(b"TITLE\0\0\0");
    bytes.extend(title.as_bytes());
    bytes.push(0);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sfo_preserves_unicode_and_punctuation_and_bounds_untrusted_fields() {
        let valid = fixture("  Pokémon [Edition].\nA & B  ");
        assert_eq!(
            sfo_title(&valid).as_deref(),
            Some("Pokémon [Edition]. A & B")
        );
        let mut version_1_0 = valid.clone();
        version_1_0[4..8].copy_from_slice(&0x100u32.to_le_bytes());
        assert_eq!(sfo_title(&version_1_0), sfo_title(&valid));
        for length in 0..valid.len() {
            assert_eq!(sfo_title(&valid[..length]), None, "truncated at {length}");
        }
        for (offset, value) in [
            (4, 0),
            (8, 20),
            (8, u32::MAX),
            (12, 30),
            (12, u32::MAX),
            (16, u32::MAX),
            (24, u32::MAX),
            (28, 0),
            (28, u32::MAX),
            (32, u32::MAX),
        ] {
            let mut broken = valid.clone();
            broken[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert_eq!(sfo_title(&broken), None, "field {offset}={value}");
        }
        let mut invalid_utf8 = fixture("Title");
        invalid_utf8[44] = 0xff;
        assert_eq!(sfo_title(&invalid_utf8), None);
        let mut integer = valid.clone();
        integer[22..24].copy_from_slice(&0x0404u16.to_le_bytes());
        assert_eq!(sfo_title(&integer), None);
        let mut no_key_terminator = valid.clone();
        no_key_terminator[36..44].fill(b'x');
        assert_eq!(sfo_title(&no_key_terminator), None);
        let mut missing_terminator = valid.clone();
        *missing_terminator.last_mut().unwrap() = b'x';
        assert_eq!(sfo_title(&missing_terminator), None);
        for title in [
            "".into(),
            "\t\n".into(),
            "A\0hidden".into(),
            "A\u{1b}B".into(),
            "X".repeat(MAX_TITLE_BYTES + 1),
        ] {
            assert_eq!(sfo_title(&fixture(&title)), None);
        }
        assert!(sfo_title(&fixture(&"X".repeat(MAX_TITLE_BYTES))).is_some());
        let mut oversized = valid.clone();
        oversized.resize(MAX_SFO_BYTES + 1, 0);
        assert_eq!(sfo_title(&oversized), None);
        let mut no_title = valid;
        no_title[36..41].copy_from_slice(b"OTHER");
        assert_eq!(sfo_title(&no_title), None);
    }

    #[test]
    fn sfo_rejects_duplicate_keys_and_checks_every_index() {
        // Two well-formed fields: TITLE text and an unrelated integer.
        let mut bytes = Vec::new();
        bytes.extend(b"\0PSF");
        for value in [0x101u32, 52, 64, 2] {
            bytes.extend(value.to_le_bytes());
        }
        for (key, format, length, offset) in [(0u16, 0x0204u16, 6u32, 0u32), (6, 0x0404, 4, 8)] {
            bytes.extend(key.to_le_bytes());
            bytes.extend(format.to_le_bytes());
            for value in [length, length, offset] {
                bytes.extend(value.to_le_bytes());
            }
        }
        bytes.extend(b"TITLE\0OTHER\0");
        bytes.extend(b"Title\0\0\0");
        bytes.extend(1u32.to_le_bytes());
        assert_eq!(sfo_title(&bytes).as_deref(), Some("Title"));
        let mut duplicate = bytes.clone();
        duplicate[36..38].copy_from_slice(&0u16.to_le_bytes());
        assert_eq!(sfo_title(&duplicate), None);
        let mut bad_tail = bytes.clone();
        bad_tail[48..52].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(sfo_title(&bad_tail), None);
        // Exercise every one-byte mutation, including arbitrary header/offset data.
        // Valid alternatives may parse; no mutation may panic or escape the limits.
        for index in 0..bytes.len() {
            for value in [0, 1, 127, 255] {
                let mut mutation = bytes.clone();
                mutation[index] = value;
                let _ = sfo_title(&mutation);
            }
        }
    }

    #[test]
    fn installed_titles_stay_local_optional_and_path_identified() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        for (id, relative, metadata) in [
            (
                "ps3",
                "Digital_Title/USRDIR/EBOOT.BIN",
                "Digital_Title/PARAM.SFO",
            ),
            (
                "ps3",
                "Disc_Title/PS3_GAME/USRDIR/EBOOT.BIN",
                "Disc_Title/PS3_GAME/PARAM.SFO",
            ),
            ("ps4", "PS4_Title/eboot.bin", "PS4_Title/sce_sys/param.sfo"),
        ] {
            let game = root.join(relative);
            let sfo = root.join(metadata);
            fs::create_dir_all(game.parent().unwrap()).unwrap();
            fs::create_dir_all(sfo.parent().unwrap()).unwrap();
            fs::write(&game, b"synthetic game fixture").unwrap();
            fs::write(&sfo, fixture("Real Title [Edition]")).unwrap();
            let games = scan_console_in(id, &[root.clone()]).unwrap();
            let row = games.iter().find(|g| Path::new(&g.uid) == game).unwrap();
            assert_eq!(row.title, "Real Title [Edition]");
            assert_eq!(row.path, game.to_string_lossy());
            assert_eq!(console_game_title(id, &game, &[root.clone()]), row.title);
            assert_eq!(title("vita", &game, &[root.clone()]), None);
            assert_eq!(title(id, &game, &[]), None);
            if id == "ps3" {
                assert_eq!(title(id, &game, &[game.parent().unwrap().to_owned()]), None);
            }
            let bytes = fs::read(&sfo).unwrap();
            assert_eq!(bytes, fixture("Real Title [Edition]"));
            fs::write(&sfo, b"invalid optional metadata").unwrap();
            let fallback = console_game_title(id, &game, &[root.clone()]);
            assert!(fallback.ends_with("Title"), "{fallback}");
            assert_eq!(fs::read(&game).unwrap(), b"synthetic game fixture");
            fs::write(&sfo, vec![0u8; MAX_SFO_BYTES + 1]).unwrap();
            assert_eq!(title(id, &game, &[root.clone()]), None);
            fs::remove_file(&sfo).unwrap();
            let outside = tempfile::NamedTempFile::new().unwrap();
            fs::write(outside.path(), fixture("Outside Title")).unwrap();
            std::os::unix::fs::symlink(outside.path(), &sfo).unwrap();
            assert_eq!(title(id, &game, &[root.clone()]), None);
            fs::remove_file(&sfo).unwrap();
            fs::create_dir(&sfo).unwrap();
            assert_eq!(title(id, &game, &[root.clone()]), None);
            if id == "ps4" {
                fs::remove_dir(&sfo).unwrap();
                fs::remove_dir(sfo.parent().unwrap()).unwrap();
                let outside_directory = tempfile::tempdir().unwrap();
                fs::write(
                    outside_directory.path().join("param.sfo"),
                    fixture("Outside directory"),
                )
                .unwrap();
                std::os::unix::fs::symlink(outside_directory.path(), sfo.parent().unwrap())
                    .unwrap();
                assert_eq!(title(id, &game, &[root.clone()]), None);
            }
        }
    }
}
