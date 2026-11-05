use regex::Regex;
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::Path,
};

/// Accept the documented Sony serial spelling and SYSTEM.CNF boot spelling only.
/// No title similarity or numeric-only guesses.
pub fn serials(text: &str) -> BTreeSet<String> {
    let re = Regex::new(r"(?i)([A-Z]{4})[-_ ]?(\d{3})\.?([0-9]{2})").unwrap();
    re.captures_iter(text)
        .filter_map(|cap| {
            let m = cap.get(0)?;
            if m.start() > 0 && text.as_bytes()[m.start() - 1].is_ascii_alphanumeric() {
                return None;
            }
            if text
                .as_bytes()
                .get(m.end())
                .is_some_and(u8::is_ascii_alphanumeric)
            {
                return None;
            }
            Some(format!("{}-{}{}", cap[1].to_uppercase(), &cap[2], &cap[3]))
        })
        .collect()
}

pub fn normalize(text: &str) -> Result<String, String> {
    let found = serials(text.trim());
    if found.len() != 1 {
        return Err("Enter one serial, for example SLUS-20312".into());
    }
    let serial = found.into_iter().next().unwrap();
    let normalized_input = text.trim().to_uppercase().replace(['_', ' ', '.'], "");
    if normalized_input != serial && normalized_input != serial.replace('-', "") {
        return Err("Enter only the game serial".into());
    }
    Ok(serial)
}

fn sector(file: &mut File, lba: u32, stride: u64, offset: u64) -> Option<[u8; 2048]> {
    file.seek(SeekFrom::Start(
        u64::from(lba).checked_mul(stride)?.checked_add(offset)?,
    ))
    .ok()?;
    let mut bytes = [0; 2048];
    file.read_exact(&mut bytes).ok()?;
    Some(bytes)
}
fn le32(bytes: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?))
}

/// Read SYSTEM.CNF from the ISO9660 root, never scan arbitrary game payloads.
/// Raw MODE1/2352 and MODE2/2352 BINs with a zero-origin data track are supported.
fn disc_serial(path: &Path, kind: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let Ok(mut file) = File::open(path) else {
        return found;
    };
    for (stride, offset) in [(2048, 0), (2352, 16), (2352, 24)] {
        let Some(pvd) = sector(&mut file, 16, stride, offset) else {
            continue;
        };
        if pvd[0] != 1 || &pvd[1..6] != b"CD001" || pvd[6] != 1 {
            continue;
        }
        let Some(root_lba) = le32(&pvd[158..162]) else {
            continue;
        };
        let Some(root_size) = le32(&pvd[166..170]) else {
            continue;
        };
        if root_size == 0 || root_size > 1024 * 1024 {
            continue;
        }
        for block in 0..root_size.div_ceil(2048) {
            let Some(data) = sector(
                &mut file,
                match root_lba.checked_add(block) {
                    Some(v) => v,
                    None => break,
                },
                stride,
                offset,
            ) else {
                break;
            };
            let mut at = 0;
            while at + 34 <= data.len() {
                let len = data[at] as usize;
                if len == 0 {
                    break;
                }
                if len < 34 || at + len > data.len() {
                    break;
                }
                let rec = &data[at..at + len];
                let name_len = rec[32] as usize;
                if let Some(name) = rec.get(33..33 + name_len) {
                    if name.eq_ignore_ascii_case(b"SYSTEM.CNF;1")
                        || name.eq_ignore_ascii_case(b"SYSTEM.CNF")
                    {
                        let size = le32(&rec[10..14]).unwrap_or(0);
                        let lba = le32(&rec[2..6]).unwrap_or(0);
                        if size > 0 && size <= 16384 && rec[25] & 2 == 0 {
                            let mut text = Vec::new();
                            for n in 0..size.div_ceil(2048) {
                                if let Some(b) = sector(
                                    &mut file,
                                    match lba.checked_add(n) {
                                        Some(v) => v,
                                        None => break,
                                    },
                                    stride,
                                    offset,
                                ) {
                                    text.extend_from_slice(&b);
                                }
                            }
                            text.truncate(size as usize);
                            for line in String::from_utf8_lossy(&text).lines() {
                                let Some((key, value)) = line.split_once('=') else {
                                    continue;
                                };
                                if key.trim().eq_ignore_ascii_case(if kind == "ps1" {
                                    "BOOT"
                                } else {
                                    "BOOT2"
                                }) {
                                    found.extend(serials(value));
                                }
                            }
                        }
                    }
                }
                at += len;
            }
        }
        break;
    }
    found
}

pub fn detect(path: &Path, kind: &str) -> Result<(String, String), String> {
    if !["ps1", "ps2"].contains(&kind) {
        return Err("Automatic covers support PS1 and PS2 only".into());
    }
    let filename = path.file_stem().and_then(|n| n.to_str()).unwrap_or("");
    let names = serials(filename);
    let mut disc = BTreeSet::new();
    match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "iso" => disc = disc_serial(path, kind),
        "cue" if kind == "ps1" => {
            // Reject ambiguous, external, nonzero-index or multi-file layouts.
            if let Ok(meta) = fs::metadata(path) {
                if meta.len() <= 65536 {
                    if let Ok(cue) = fs::read_to_string(path) {
                        let files: Vec<_> = cue
                            .lines()
                            .filter(|l| l.trim().to_ascii_uppercase().starts_with("FILE "))
                            .collect();
                        let zero_data = cue.to_ascii_uppercase().contains("INDEX 01 00:00:00");
                        if files.len() == 1 && zero_data {
                            if let Some(name) = files[0].split('"').nth(1) {
                                if let Some(parent) = path.parent() {
                                    let root = parent.canonicalize().ok();
                                    let target = parent.join(name).canonicalize().ok();
                                    if let (Some(root), Some(target)) = (root, target) {
                                        if target.starts_with(root) {
                                            disc = disc_serial(&target, kind);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        _ => (), // Compressed formats require filename serial or user confirmation.
    }
    let all: BTreeSet<_> = names.union(&disc).cloned().collect();
    if all.len() > 1 {
        return Err("Conflicting serials; confirm the correct serial in game details".into());
    }
    let serial = all
        .into_iter()
        .next()
        .ok_or("No serial found; confirm a serial in game details or choose a local cover")?;
    Ok((
        serial,
        if disc.is_empty() {
            "filename"
        } else {
            "SYSTEM.CNF"
        }
        .into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_normalization() {
        for v in ["SLUS-20312", "slus_203.12", "SLUS20312"] {
            assert_eq!(normalize(v).unwrap(), "SLUS-20312");
        }
        for v in [
            "../../SLUS-20312",
            "20312",
            "SLUS-203120",
            "Title SLUS-20312",
            "",
        ] {
            assert!(normalize(v).is_err(), "{v}");
        }
        assert_eq!(serials("Game (SLUS-20312) (SLES-12345)").len(), 2);
        assert!(serials("ASLUS-20312Z").is_empty());
    }
    #[test]
    fn no_title_guess_and_ambiguity() {
        assert!(detect(Path::new("Final Fantasy X.iso"), "ps2").is_err());
        assert!(detect(Path::new("SLUS-20312 SLES-12345.chd"), "ps2").is_err());
        assert!(detect(Path::new("SLUS-20312.iso"), "ps3").is_err());
        assert_eq!(
            detect(Path::new("Game [SLUS_203.12].chd"), "ps2")
                .unwrap()
                .0,
            "SLUS-20312"
        );
    }
    pub fn fixture(path: &Path, boot: &str, stride: usize, offset: usize) {
        let mut disc = vec![0u8; 24 * stride];
        let p = 16 * stride + offset;
        disc[p] = 1;
        disc[p + 1..p + 6].copy_from_slice(b"CD001");
        disc[p + 6] = 1;
        disc[p + 158..p + 162].copy_from_slice(&20u32.to_le_bytes());
        disc[p + 166..p + 170].copy_from_slice(&2048u32.to_le_bytes());
        let r = 20 * stride + offset;
        disc[r] = 46;
        disc[r + 2..r + 6].copy_from_slice(&21u32.to_le_bytes());
        disc[r + 10..r + 14].copy_from_slice(&(boot.len() as u32).to_le_bytes());
        disc[r + 32] = 12;
        disc[r + 33..r + 45].copy_from_slice(b"SYSTEM.CNF;1");
        let b = 21 * stride + offset;
        disc[b..b + boot.len()].copy_from_slice(boot.as_bytes());
        fs::write(path, disc).unwrap();
    }
    #[test]
    fn read_iso_and_cue_boot_not_payload() {
        let tmp = tempfile::tempdir().unwrap();
        let iso = tmp.path().join("Game.iso");
        fixture(&iso, "BOOT2 = cdrom0:\\SLUS_203.12;1\n", 2048, 0);
        assert_eq!(
            detect(&iso, "ps2").unwrap(),
            ("SLUS-20312".into(), "SYSTEM.CNF".into())
        );
        assert!(detect(&iso, "ps1").is_err());
        let bad = tmp.path().join("SLES-12345.iso");
        fs::copy(&iso, &bad).unwrap();
        assert!(detect(&bad, "ps2").is_err());
        let bin = tmp.path().join("track.bin");
        fixture(&bin, "BOOT = cdrom:\\SLUS_006.64;1\n", 2352, 24);
        let cue = tmp.path().join("Game.cue");
        fs::write(
            &cue,
            "FILE \"track.bin\" BINARY\n TRACK 01 MODE2/2352\n INDEX 01 00:00:00",
        )
        .unwrap();
        assert_eq!(detect(&cue, "ps1").unwrap().0, "SLUS-00664");
        fs::write(&cue, "FILE \"../track.bin\" BINARY\n INDEX 01 00:00:00").unwrap();
        assert!(detect(&cue, "ps1").is_err());
    }
}
