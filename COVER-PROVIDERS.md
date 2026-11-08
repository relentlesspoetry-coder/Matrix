# Matrix Play cover providers — reviewed November 5, 2026

## Version 0.5.16: extracted PS5 local icons

PS5 remains without an online cover provider. A validated extracted `eboot.bin`
entry in a dedicated folder can use `sce_sys/icon0.png`, after imported covers,
same-stem PNG/JPEG/WebP images and generic `cover.*` images. The folder must be
inside a registered library, not the library root, and contain exactly one
recognized boot target. Additional ELF/ZAR targets suppress folder-wide art
without hiding any games. Other consoles and non-eboot PS5 files do not use this
icon path. Files are decoded and capped at 4 MiB, including the read itself;
symlinks in the icon path, nonregular files and corrupt/oversized images are
ignored. `param.json` is not interpreted. This adds no network requests, arbitrary
metadata paths, encrypted-content extraction or gameplay compatibility claim.

## Version 0.5.16 — original Switch product-code covers, reviewed November 8, 2026

Switch now has a bounded GameTDB provider. This makes **16 of 18 libraries**
provider-capable, not sixteen complete catalogs. **PS5 and Switch 2 still require
local artwork; extracted PS5 icons can now be discovered automatically.** Most Switch files named only with a sixteen-digit title ID need
one explicit product-code/region confirmation; this release does not map those
IDs automatically or read encrypted game content.

[GameTDB's Switch Downloads page](https://www.gametdb.com/Switch/Downloads)
documents the Switch database and artwork packs, and its
[homepage](https://www.gametdb.com/) says downloads need no signup. Indexed
first-party text was available; direct website requests returned 403. No HTML
scraper, access workaround, bulk download or claim about unknown robots rules
is implemented. These access examples are not a copyright/redistribution grant.
No provider-specific numerical rate limit was established. Existing conservative
serialization (at least two seconds between requests), seven-day misses,
Retry-After/403/429/503 cooldowns and validated bounded HTTPS/image handling apply.

The actual artwork host returned HTTP 200 for
`https://art.gametdb.com/switch/coverM/US/AAACA.jpg`: a decoded 352×570 JPEG showing
Super Mario Odyssey, consistent with [GameTDB's AAACA entry](https://www.gametdb.com/Switch/AAACA).
The `box/US/AAACA.png` image was also inspected (300×486): it is a front-facing
case, not an angled 3D cover. Only the verified `coverM/{region}/{product}.jpg`
family is implemented. The 3D preference reuses the flat cover/cache and clearly
states the fallback. There is no guessed 3D endpoint or alternate region request.

- Product identity is an exact five-character alphanumeric GameTDB code plus a
  required artwork region, e.g. `AAACA-US`. Filename lookup accepts a complete
  code-only stem or an explicit bracket/parenthesis token, e.g.
  `Game [HAC-P-AAACA-USA].nsp`. Ordinary title words are not searched.
- Full original-Switch packaging prefixes `HAC-P-` and `LA-H-` are accepted.
  Explicit USA/EUR/JPN/AUS/KOR labels map to US/EN/JA/AU/KO. EUR intentionally
  selects English European artwork and the UI discloses this. Other supported
  explicit provider regions are FR, DE, ES, IT, NL, PT, RU, ZHCN and ZHTW.
  Missing region, conflicting codes/regions and Switch 2 markers require review.
- Sixteen-digit title IDs are never truncated, converted or sent as product IDs.
  Switch 2 product codes can collide with original Switch codes, so Switch 2
  remains separate and never borrows this provider. No header keys or game bytes
  are read/uploaded for this new identity path.
- Only the constrained product code and selected artwork region/style go to
  GameTDB. No title, filename or local path is sent. Confirmations expire when
  the file size/modification time changes. Local/imported covers remain first
  priority, including explicit Find exact cover actions. Cache keys include the
  console and region; misses never silently fall back to another region.

### Remaining source gaps

[Switchbrew's Switch 2 title list](https://www.switchbrew.org/wiki/Switch_2:_Title_list/Games)
has IDs and names but no supported cover endpoint. [TitleDB issue 18](https://github.com/blawar/titledb/issues/18)
describes missing Switch 2 IDs, and [Nlib's own documentation](https://github.com/ghost-land/Nlib-API)
only documents its original Switch `01` catalog. A Switch 1 icon is not evidence
of a Switch 2 edition's cover. No verified PS5 PPSA-to-cover public API or stable
provider-supported unauthenticated catalog was found; unofficial PlayStation
Store scraping is not implemented.

[IGDB's official API](https://api-docs.igdb.com/) is a possible future
platform-aware search provider, requiring Twitch app credentials/OAuth, with
4 requests/second and 8 concurrent requests and applicable developer terms.
[SteamGridDB's official client](https://github.com/SteamGridDB/node-steamgriddb)
also requires an API key. Neither has been established here as an exact
PPSA/0400-title-ID replacement. No credentials are embedded, new account access
is not created, and these sources are not claimed as implemented coverage.

Regression fixtures cover explicit identities, invalid inputs, conflicting
regions/generations, no-request title-ID misses, persisted confirmation and
invalidation, separate caches and custom-cover precedence. CI now requires the
production Rust Switch download smoke test before installer builds, and the
real native online suite tests automatic Switch download plus the exact-code UI
and retained custom art. All game fixtures are synthetic. Live sample success
is not universal catalog or gameplay verification.


## Version 0.5.15 extension — Libretro, reviewed November 8, 2026

This adds a title-based provider for eleven more Matrix libraries: PS3, PS4,
PSP, Vita, N64, Dreamcast, Wii U, Game Boy/Game Boy Color, GBA, DS and 3DS.
Together with the four existing ID-based libraries, fifteen of the eighteen
libraries have an automatic provider. **Switch, PS5 and Switch 2 still need
local artwork.** No complete all-console coverage claim is made.

The first-party [repository README](https://github.com/libretro-thumbnails/libretro-thumbnails)
documents the directory/name convention and download use. The current
[thumbnail server index](https://thumbnails.libretro.com/) lists the mapped
catalogs. Requests use HTTPS and the fixed template
`https://thumbnails.libretro.com/{encoded system}/Named_Boxarts/{encoded title}.png`.
The static mapping lives in `cover-core/src/title.rs`; GB and GBC are distinct
provider directories within Matrix's combined Game Boy library. No directory
scraping, fuzzy search or bulk catalog download is implemented.

- Automatic title-provider scans require a separate explicit Settings opt-in,
  including for existing installations whose earlier automatic covers were on.
  This is necessary because the new requests disclose a filename stem, rather
  than only a constrained serial. An individual Find exact cover action sends
  the explicitly entered title without enabling future scans.
- Names preserve region, language, edition and revision tags. Only the provider's
  documented punctuation substitutions are made; Unicode and URL metacharacters
  are encoded as a single path component. No cleaned display title, guessed
  region, PS4-to-PS5 fallback or Switch-to-Switch-2 fallback is used.
- ID-only filenames and generic executable names such as eboot require manual
  title confirmation. Installed-game display metadata is not treated as an
  exact Libretro catalog name. Unknown titles return a cached miss; users can
  enter a corrected catalog title or import an image.
- Only `Named_Boxarts` is used. This provider has flat box covers, not 3D artwork.
  Selecting 3D reuses its default-cover cache and clearly labels that fallback.
- Imported/local covers remain first priority. New entries reuse the serialized
  queue, bounded transport/decoding, negative caching and offline persistence.
  Existing serial-provider cache keys do not change. User-confirmed title
  bindings expire when the game file's size or modification time changes.
- Catalog availability is not completeness. The inspected Vita boxart directory
  contained nine images. Separate PS3 downloadable and 3DS digital directories
  exist but are not silently queried as fallback catalogs in this implementation.
- Public access and documented downloader use do not establish a blanket
  copyright or redistribution license for commercial artwork. No downloaded
  artwork or user game data is included in this source tree.

Provider audit also found a GameTDB Switch catalog, but its five-character
product IDs (for example AAACA) are not Switch's sixteen-digit title IDs.
The exact image style/extension and safe ID mapping remain to be verified
before implementation. No verified PS5 or Switch 2 template was established.

Fixture tests pass for mappings, exact names, confirmation, cache boundaries,
legacy settings, local-cover precedence and disabled-provider behavior. The
initial local production Rust live check failed with a transport error; HTTPS
validation was not changed. The cover candidate then passed required live
Libretro downloads, existing live providers, native Settings opt-in/out and
custom-cover preservation, both installer builds and repeated native/AppImage
checks in [CI run 42436367853](https://github.com/relentlesspoetry-coder/Matrix/actions/runs/42436367853).
The combined versioned release must repeat these gates before publication.

## xlenore: implemented

The [PS1 repository README](https://github.com/xlenore/psx-covers#duckstation-setup) and [PS2 repository README](https://github.com/xlenore/ps2-covers#pcsx2-setup) explicitly document emulator cover-downloader URLs. Both match the screenshots supplied with this request:

| Console | Default | 3D |
|---|---|---|
| PS1 | `https://raw.githubusercontent.com/xlenore/psx-covers/main/covers/default/${serial}.jpg` | `https://raw.githubusercontent.com/xlenore/psx-covers/main/covers/3d/${serial}.png` |
| PS2 | `https://raw.githubusercontent.com/xlenore/ps2-covers/main/covers/default/${serial}.jpg` | `https://raw.githubusercontent.com/xlenore/ps2-covers/main/covers/3d/${serial}.png` |

`${serial}` is replaced with a normalized serial, such as `SLUS-20312`. The raw READMEs were also inspected to avoid the space inserted into `3d` by some rendered search results. Matrix Play uses the exact `3d` directory.

These documented downloader instructions support the selected access method. They are **not a blanket copyright license** for underlying commercial artwork. No additional copyright/redistribution grant was established by this review. Matrix Play saves images locally for the user's library; the source archive contains no artwork retrieved during live provider tests. This is an engineering access decision, not a determination of an individual user's legal rights.

One explicit live test downloaded and decoded the default and 3D images for PS1 `SLUS-00664` and PS2 `SLUS-20312`, then removed the temporary files. This verifies four sample URLs, not universal repository coverage or correctness of all provider metadata.

## TheCoverProject: manual only; automation authorization unconfirmed

Reviewed the user-supplied [category URL](https://www.thecoverproject.net/view.php?cat_id=19), [home page](https://www.thecoverproject.net/), [robots.txt](https://www.thecoverproject.net/robots.txt), and attempted official FAQ/contact/forum pages, along with searches for official API, copyright, scraping and bot policies.

Findings:

- Direct retrieval of the category and homepage returned HTTP 403 in the research tool.
- robots.txt and the attempted FAQ/contact/forum pages could not be retrieved. Their contents and applicable directives remain **unknown**.
- No current first-party API documentation, express automation permission, applicable scraping terms, or artwork copyright license was established.
- Search results showing individual cover listings establish that a catalogue exists; they do not establish permission for Matrix Play automation. No conclusions were drawn from unrelated third-party scrapers or old forum speculation.
- The earlier chat's statement that `cat_id=19` is PS3 was not independently confirmed during this review. Matrix Play does not use that category ID for matching.

Decision: **do not implement or enable a scraper/provider** on this evidence. Unknown permission is not a finding that all automation is prohibited, nor does HTTP 403 by itself establish a legal prohibition. It is insufficient to satisfy this request's “if permitted” condition.

The UI has an explicit button opening the official homepage in the user's browser, and Choose cover saves a downloaded local image. Matrix Play sends no automated catalogue/search/download requests to this service, and does not solve challenges, rotate identities, or use alternate domains. Users must identify the right console, title, region and artwork themselves when importing. Manual import does not establish copyright permission either.

Before adding automation later, obtain current first-party authorization/API instructions, verify robots and applicable terms, and establish the intended artwork use. Any future provider needs its own console identifiers, exact-title/region/edition candidate review, persistent caching, a single request queue, service-specified limits and Retry-After handling. These future capabilities are **not implemented**.

## Matching and persistence in v0.5

- Console is selected by the existing dedicated library; PS1 and PS2 never share a provider cache entry.
- Standard four-letter/five-digit Sony serials are recognized in filenames. Case and common `SLUS_203.12` spelling are normalized. Filename-derived identity is labeled as such, not described as a verified disc identity.
- For uncompressed ISO9660 images, Matrix Play reads the root SYSTEM.CNF's BOOT (PS1) or BOOT2 (PS2) value. It supports cooked 2048-byte ISO sectors and zero-origin MODE1/MODE2 2352-byte raw sectors through simple one-file CUEs. It does not search arbitrary binary payloads for serial-looking text.
- Missing or conflicting serials produce a review status without any network request. No fuzzy title lookup, cross-region fallback, alternate-disc guess or cross-console fallback occurs.
- CHD, CSO, PBP, CCD and compressed GZ content parsing is not implemented. Use a serial in the filename, explicitly confirm one in details, or import an image. Unusual serial formats are left unresolved.
- User-confirmed serials persist, with an explicit identity source. File size and modification time invalidate identity bindings after normal file replacements; CUE bindings also include referenced track metadata. This is change detection, not a full-ROM checksum or authenticity check.
- Local/manual covers win. A generic `cover.*` is used only in a console game subfolder containing one recognized game file, preventing one shared folder image from being applied to a whole library. Exact file-stem image matching remains available.
- Download cache keys include console, serial and style. Successful images persist under `$XDG_DATA_HOME/matrix/covers/online/` with a JSON provenance/index file. Files remain usable while offline or with automatic downloading disabled.
- Requests are serialized across threads/processes with a file lock, with at least two seconds between request starts. A 404 is cached seven days; transient failures and invalid content are cached fifteen minutes; transport failure also opens a one-minute provider cooldown. 403 opens a 24-hour cooldown; 429/503 default to one hour. Retry-After seconds and HTTP dates are honored with a minimum one-minute cooldown. A later Refresh retries expired failures; an in-progress request may finish after automatic downloads are disabled.
- TLS validation stays enabled, redirects are rejected, only fixed GitHub URL templates are used, network reads are capped at 4 MB, timeouts are bounded, and images must successfully decode within 4096×4096 dimensions. Invalid or corrupt cache images are not displayed; a later scan replaces corrupt files.

## GameTDB: GameCube and Wii exact-ID covers

Added as a bounded extension of automatic artwork, not an emulator compatibility
claim. [GameTDB's Wii & GameCube Downloads page](https://www.gametdb.com/Wii/Downloads)
documents downloadable artwork and programs that use it. The first-party page's
indexed text was reviewed; direct HTML access through the research browser
returned 403, and robots.txt could not be retrieved. No HTML scraping or attempt
to work around those responses is implemented. Dolphin's
[official cover downloader](https://github.com/dolphin-emu/dolphin/blob/master/Source/Core/UICommon/GameFile.cpp)
also uses the fixed GameTDB artwork host. These access examples do not grant a
copyright or redistribution license for commercial artwork.

Matrix Play uses `https://art.gametdb.com/wii/cover/{region}/{id}.png` for Default
and `https://art.gametdb.com/wii/cover3D/{region}/{id}.png` for 3D. Both GameCube
and Wii artwork reside under the provider's `wii` directory; Matrix keeps their
cache entries separate. Six-character IDs are platform-scoped: GameCube retail
G/D prefixes and Wii R/S prefixes. Other formats remain manual-only.
The fourth ID character maps E→US, J→JA, P→EN, D→DE, F→FR, I→IT, S→ES and K→KO.
Unknown regions are rejected rather than trying unrelated regional artwork.
PAL P selects the provider's English artwork without changing the disc ID.

- Uncompressed ISO/GCM files use only the first 32 bytes, validate the expected
  GameCube/Wii magic and read the six-byte disc ID. Conflicting platform markers,
  malformed IDs, unsupported regions and filename/header conflicts require review.
- Filename identities require a bracketed ID (`Title [RMGE01].rvz`, or parentheses)
  or a filename consisting only of the ID. Ordinary title text is never searched
  for a guessed six-character match. These are labeled filename identities, not
  verified disc headers.
- RVZ, WBFS, WIA, GCZ and other compressed formats are not parsed for identity.
  Use an explicit filename ID, confirm the exact ID in details, or choose a local
  image. This addition does not expand which game formats the launcher accepts.
- User-confirmed IDs and metadata invalidation reuse the existing cache format.
  Local covers retain priority. Shared serialized requests, TLS validation,
  rejected redirects, bounded reads/decoding, negative caching and Retry-After
  handling apply to GameTDB as well. Only the game ID, selected region/style and
  ordinary request metadata are sent; no local path or game bytes are uploaded.
- Artwork support for the other libraries remains unchanged. Missing provider
  images still require a local cover; supported identity formats do not promise
  complete catalogue coverage.

Tests use synthetic 32-byte headers and fixture images, never game content.

Validation on November 7, 2026: all 16 ordinary cover-core tests passed (two
opt-in live tests excluded). Direct HTTPS samples for GameCube GMSE01 Default
and Wii RMGE01 3D returned HTTP 200 and decoded as PNGs. The production Rust
transport's opt-in GameTDB smoke test could not complete in this executor:
`InvalidCertificate(UnknownIssuer)`. The same diagnostic occurred for the
existing xlenore PS1 endpoint. TLS verification was not weakened. Run
`cargo test --locked --manifest-path cover-core/Cargo.toml live_gametdb_templates -- --ignored`
in a normal trusted environment before claiming end-to-end production-network
verification. Two successful sample URLs do not establish universal coverage.
