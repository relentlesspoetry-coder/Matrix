//! Original Switch product codes, not title IDs or Switch 2 product codes.
//! The artwork region is mandatory: a five-character code does not encode it.
use regex::Regex;
use std::{collections::BTreeSet, path::Path};

pub fn normalize(input: &str) -> Result<String, String> {
    let input = input.trim().to_ascii_uppercase();
    let value = input
        .strip_prefix("HAC-P-")
        .or_else(|| input.strip_prefix("LA-H-"))
        .unwrap_or(&input);
    let (id, region) = value.split_once('-').ok_or(
        "Enter the Switch product code and artwork region, for example AAACA-US (not a 16-digit title ID)",
    )?;
    if id.len() != 5 || !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err(
            "Enter a five-character original Switch product code and artwork region".into(),
        );
    }
    let region = match region {
        "USA" => "US",
        "EUR" => "EN", // Explicit European packaging selects GameTDB's English artwork.
        "JPN" => "JA",
        "AUS" => "AU",
        "KOR" => "KO",
        "US" | "EN" | "JA" | "AU" | "KO" | "FR" | "DE" | "ES" | "IT" | "NL" | "PT" | "RU"
        | "ZHCN" | "ZHTW" => region,
        _ => return Err("Choose an explicit GameTDB artwork region, such as US, EN or JA".into()),
    };
    Ok(format!("{id}-{region}"))
}

pub fn url(input: &str) -> Result<String, String> {
    let identity = normalize(input)?;
    let (id, region) = identity.split_once('-').unwrap();
    Ok(format!(
        "https://art.gametdb.com/switch/coverM/{region}/{id}.jpg"
    ))
}

pub fn detect(path: &Path) -> Result<(String, String), String> {
    let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    // Product codes are reused between Switch generations. Never map BEE/LB to HAC.
    // Match a product-code token, not a suffix inside an ordinary title such as
    // "MLB-The Show". Spaces, brackets and filename separators remain boundaries.
    if Regex::new(r"(?i)(?:^|[^a-z0-9])(?:BEE-P-|LB-)")
        .unwrap()
        .is_match(name)
        || Regex::new(r"(?i)(?:^|[^a-z0-9])0400[0-9a-f]{12}(?:$|[^a-z0-9])")
            .unwrap()
            .is_match(name)
    {
        return Err(
            "This filename indicates Switch 2; original Switch artwork is not a fallback".into(),
        );
    }
    let mut ids = BTreeSet::new();
    if let Ok(id) = normalize(name) {
        ids.insert(id);
    }
    for capture in Regex::new(r"\[([^\[\]]+)\]|\(([^()]+)\)")
        .unwrap()
        .captures_iter(name)
    {
        let token = capture.get(1).or_else(|| capture.get(2)).unwrap().as_str();
        if let Ok(id) = normalize(token) {
            ids.insert(id);
        } else if token.to_ascii_uppercase().starts_with("HAC-P-")
            || token.to_ascii_uppercase().starts_with("LA-H-")
        {
            return Err("The Switch product code needs an explicit artwork region, for example HAC-P-AAACA-USA".into());
        }
    }
    if ids.len() > 1 {
        return Err(
            "Conflicting Switch product codes or regions; confirm the correct one in game details"
                .into(),
        );
    }
    let id = ids.into_iter().next().ok_or(
        "No exact Switch product code and region found. Confirm AAACA-US or a matching code in game details; 16-digit title IDs cannot be converted safely",
    )?;
    Ok((id, "filename-product-code".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn product_codes_require_explicit_region_and_platform() {
        for (input, expected) in [
            (" aaaca-us ", "AAACA-US"),
            ("HAC-P-AAACA-USA", "AAACA-US"),
            ("LA-H-AAACA-EUR", "AAACA-EN"),
            ("AAACA-JPN", "AAACA-JA"),
            ("ABC12-ZHTW", "ABC12-ZHTW"),
        ] {
            assert_eq!(normalize(input).unwrap(), expected);
        }
        for input in [
            "AAACA",
            "0100000000010000",
            "BEE-P-AAACA-USA",
            "LB-AAACA-USA",
            "AAACA-XX",
            "AAACA-US/../EN",
            "../AAACA-US",
            "https://host/AAACA-US",
            "AAACA-US?x",
            "AAACA-US\nX",
        ] {
            assert!(normalize(input).is_err(), "{input}");
        }
        assert_eq!(
            url("HAC-P-AAACA-USA").unwrap(),
            "https://art.gametdb.com/switch/coverM/US/AAACA.jpg"
        );
        assert_ne!(url("AAACA-US").unwrap(), url("AAACA-EN").unwrap());
    }
    #[test]
    fn filenames_are_explicit_not_fuzzy_or_title_id_conversions() {
        for name in [
            "Game [HAC-P-AAACA-USA].nsp",
            "Game (AAACA-US).xci",
            "LA-H-AAACA-USA.nsz",
        ] {
            assert_eq!(
                detect(Path::new(name)).unwrap(),
                ("AAACA-US".into(), "filename-product-code".into())
            );
        }
        for name in [
            "Super Mario Odyssey.nsp",
            "Game [0100000000010000].nsp",
            "Game [AAACA].nsp",
            "Game AAACA-US.nsp",
            "Game [HAC-P-AAACA].nsp",
            "Game [AAACA-US] [AAACA-EN].nsp",
            "Game [AAACA-US] [ABAAA-US].nsp",
            "Game [BEE-P-AAACA-USA] [AAACA-US].nsp",
            "Game [040074A01BF12000] [AAACA-US].nsp",
        ] {
            assert!(detect(Path::new(name)).is_err(), "{name}");
        }
    }

    #[test]
    fn successor_markers_must_start_a_product_code_token() {
        for name in [
            "MLB-The Show [HAC-P-AAACA-USA].nsp",
            "MLB-Game (AAACA-US).xci",
            "Freebee-P-Game [AAACA-US].nsp",
        ] {
            assert_eq!(detect(Path::new(name)).unwrap().0, "AAACA-US", "{name}");
        }
        for name in [
            "LB-AAACA-USA [AAACA-US].nsp",
            "Game_LB-AAACA-USA [AAACA-US].nsp",
            "Game [LB-AAACA-USA] [AAACA-US].nsp",
            "Game (BEE-P-AAACA-USA) [AAACA-US].nsp",
            "Game [bee-p-aaaca-usa] [AAACA-US].nsp",
        ] {
            assert!(detect(Path::new(name)).is_err(), "{name}");
        }
    }
}
