//! Exact Libretro thumbnail names. No fuzzy search or cross-platform fallback.
use std::path::Path;

pub fn system(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "ps3" => "Sony - PlayStation 3",
        "ps4" => "Sony - PlayStation 4",
        "psp" => "Sony - PlayStation Portable",
        "vita" => "Sony - PlayStation Vita",
        "n64" => "Nintendo - Nintendo 64",
        "dreamcast" => "Sega - Dreamcast",
        "wiiu" => "Nintendo - Wii U",
        "gb" => "Nintendo - Game Boy",
        "gbc" => "Nintendo - Game Boy Color",
        "gba" => "Nintendo - Game Boy Advance",
        "nds" => "Nintendo - Nintendo DS",
        "3ds" => "Nintendo - Nintendo 3DS",
        _ => return None,
    })
}

/// Matrix has one combined GB/GBC library, but the provider has separate catalogs.
pub fn platform<'a>(kind: &'a str, path: &Path) -> Result<&'a str, String> {
    if kind != "gb" {
        return Ok(kind);
    }
    match path.extension().and_then(|v| v.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("gb") => Ok("gb"),
        Some("gbc") => Ok("gbc"),
        _ => Err("Cannot distinguish Game Boy from Game Boy Color in this container; choose a local cover".into()),
    }
}

pub fn normalize(text: &str) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty()
        || text.len() > 240
        || matches!(text, "." | "..")
        || text.chars().any(char::is_control)
        || text.starts_with(['/', '\\'])
        || text.contains("://")
        || text.as_bytes().get(1) == Some(&b':')
        || text
            .split(['/', '\\'])
            .any(|part| matches!(part, "." | ".."))
    {
        return Err(
            "Enter the exact catalog title, including region and edition, without a path".into(),
        );
    }
    // Documented Libretro filename substitutions, not title/region normalization.
    Ok(text
        .chars()
        .map(|c| if "&*:<>?\"|`/\\".contains(c) { '_' } else { c })
        .collect())
}

fn component(text: &str) -> String {
    let mut encoded = String::new();
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            encoded.push(b as char);
        } else {
            encoded.push_str(&format!("%{b:02X}"));
        }
    }
    encoded
}

pub fn url(kind: &str, name: &str) -> Result<String, String> {
    let system = system(kind).ok_or("No title catalog for this console")?;
    let name = normalize(name)?;
    Ok(format!(
        "https://thumbnails.libretro.com/{}/Named_Boxarts/{}.png",
        component(system),
        component(&name)
    ))
}

pub fn detect(path: &Path) -> Result<(String, String), String> {
    let name = path
        .file_stem()
        .and_then(|v| v.to_str())
        .ok_or("Cannot read the game's title")?;
    if ["eboot", "boot", "game", "main", "default", "disc", "rom"]
        .contains(&name.to_ascii_lowercase().as_str())
        || name.bytes().all(|b| b.is_ascii_hexdigit())
        || crate::identity::normalize(name).is_ok()
    {
        return Err(
            "No exact catalog title in this filename; confirm a title in game details".into(),
        );
    }
    Ok((normalize(name)?, "filename-title".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_names_preserve_region_edition_and_unicode() {
        assert_eq!(
            normalize("  Pokémon - Emerald (USA, Europe) (Rev 1)  ").unwrap(),
            "Pokémon - Emerald (USA, Europe) (Rev 1)"
        );
        assert_eq!(normalize("Title: A & B?").unwrap(), "Title_ A _ B_");
        assert_eq!(normalize("Fate/Extra").unwrap(), "Fate_Extra");
        assert_eq!(normalize("Title`One\\Two").unwrap(), "Title_One_Two");
        let url = url("gba", "Pokémon #1 (USA)").unwrap();
        assert!(url.ends_with("Pok%C3%A9mon%20%231%20%28USA%29.png"));
        for input in [
            "",
            "..",
            "../Title",
            "C:\\games",
            "https://host/a",
            "Title\nUSA",
        ] {
            assert!(normalize(input).is_err(), "{input}");
        }
        assert!(normalize(&"a".repeat(241)).is_err());
    }

    #[test]
    fn filename_detection_never_cleans_or_guesses_titles() {
        assert_eq!(
            detect(Path::new("Sonic Adventure (USA) (En,Fr).chd")).unwrap(),
            (
                "Sonic Adventure (USA) (En,Fr)".into(),
                "filename-title".into()
            )
        );
        for name in ["eboot.bin", "0100000000000000.nsp", "GAME.iso"] {
            assert!(detect(Path::new(name)).is_err());
        }
        assert_eq!(platform("gb", Path::new("Title.GBC")).unwrap(), "gbc");
        assert_eq!(platform("gb", Path::new("Title.gb")).unwrap(), "gb");
        assert!(platform("gb", Path::new("Title.zip")).is_err());
        assert!(system("ps5").is_none());
        assert!(system("switch2").is_none());
    }
}
