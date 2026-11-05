# Matrix cover providers — reviewed November 5, 2026

## xlenore: implemented

The [PS1 repository README](https://github.com/xlenore/psx-covers#duckstation-setup) and [PS2 repository README](https://github.com/xlenore/ps2-covers#pcsx2-setup) explicitly document emulator cover-downloader URLs. Both match the screenshots supplied with this request:

| Console | Default | 3D |
|---|---|---|
| PS1 | `https://raw.githubusercontent.com/xlenore/psx-covers/main/covers/default/${serial}.jpg` | `https://raw.githubusercontent.com/xlenore/psx-covers/main/covers/3d/${serial}.png` |
| PS2 | `https://raw.githubusercontent.com/xlenore/ps2-covers/main/covers/default/${serial}.jpg` | `https://raw.githubusercontent.com/xlenore/ps2-covers/main/covers/3d/${serial}.png` |

`${serial}` is replaced with a normalized serial, such as `SLUS-20312`. The raw READMEs were also inspected to avoid the space inserted into `3d` by some rendered search results. Matrix uses the exact `3d` directory.

These documented downloader instructions support the selected access method. They are **not a blanket copyright license** for underlying commercial artwork. No additional copyright/redistribution grant was established by this review. Matrix saves images locally for the user's library; the source archive contains no artwork retrieved during live provider tests. This is an engineering access decision, not a determination of an individual user's legal rights.

One explicit live test downloaded and decoded the default and 3D images for PS1 `SLUS-00664` and PS2 `SLUS-20312`, then removed the temporary files. This verifies four sample URLs, not universal repository coverage or correctness of all provider metadata.

## TheCoverProject: manual only; automation authorization unconfirmed

Reviewed the user-supplied [category URL](https://www.thecoverproject.net/view.php?cat_id=19), [home page](https://www.thecoverproject.net/), [robots.txt](https://www.thecoverproject.net/robots.txt), and attempted official FAQ/contact/forum pages, along with searches for official API, copyright, scraping and bot policies.

Findings:

- Direct retrieval of the category and homepage returned HTTP 403 in the research tool.
- robots.txt and the attempted FAQ/contact/forum pages could not be retrieved. Their contents and applicable directives remain **unknown**.
- No current first-party API documentation, express automation permission, applicable scraping terms, or artwork copyright license was established.
- Search results showing individual cover listings establish that a catalogue exists; they do not establish permission for Matrix automation. No conclusions were drawn from unrelated third-party scrapers or old forum speculation.
- The earlier chat's statement that `cat_id=19` is PS3 was not independently confirmed during this review. Matrix does not use that category ID for matching.

Decision: **do not implement or enable a scraper/provider** on this evidence. Unknown permission is not a finding that all automation is prohibited, nor does HTTP 403 by itself establish a legal prohibition. It is insufficient to satisfy this request's “if permitted” condition.

The UI has an explicit button opening the official homepage in the user's browser, and Choose cover saves a downloaded local image. Matrix sends no automated catalogue/search/download requests to this service, and does not solve challenges, rotate identities, or use alternate domains. Users must identify the right console, title, region and artwork themselves when importing. Manual import does not establish copyright permission either.

Before adding automation later, obtain current first-party authorization/API instructions, verify robots and applicable terms, and establish the intended artwork use. Any future provider needs its own console identifiers, exact-title/region/edition candidate review, persistent caching, a single request queue, service-specified limits and Retry-After handling. These future capabilities are **not implemented**.

## Matching and persistence in v0.5

- Console is selected by the existing dedicated library; PS1 and PS2 never share a provider cache entry.
- Standard four-letter/five-digit Sony serials are recognized in filenames. Case and common `SLUS_203.12` spelling are normalized. Filename-derived identity is labeled as such, not described as a verified disc identity.
- For uncompressed ISO9660 images, Matrix reads the root SYSTEM.CNF's BOOT (PS1) or BOOT2 (PS2) value. It supports cooked 2048-byte ISO sectors and zero-origin MODE1/MODE2 2352-byte raw sectors through simple one-file CUEs. It does not search arbitrary binary payloads for serial-looking text.
- Missing or conflicting serials produce a review status without any network request. No fuzzy title lookup, cross-region fallback, alternate-disc guess or cross-console fallback occurs.
- CHD, CSO, PBP, CCD and compressed GZ content parsing is not implemented. Use a serial in the filename, explicitly confirm one in details, or import an image. Unusual serial formats are left unresolved.
- User-confirmed serials persist, with an explicit identity source. File size and modification time invalidate identity bindings after normal file replacements; CUE bindings also include referenced track metadata. This is change detection, not a full-ROM checksum or authenticity check.
- Local/manual covers win. A generic `cover.*` is used only in a console game subfolder containing one recognized game file, preventing one shared folder image from being applied to a whole library. Exact file-stem image matching remains available.
- Download cache keys include console, serial and style. Successful images persist under `$XDG_DATA_HOME/matrix/covers/online/` with a JSON provenance/index file. Files remain usable while offline or with automatic downloading disabled.
- Requests are serialized across threads/processes with a file lock, with at least two seconds between request starts. A 404 is cached seven days; transient failures and invalid content are cached fifteen minutes; transport failure also opens a one-minute provider cooldown. 403 opens a 24-hour cooldown; 429/503 default to one hour. Retry-After seconds and HTTP dates are honored with a minimum one-minute cooldown. A later Refresh retries expired failures; an in-progress request may finish after automatic downloads are disabled.
- TLS validation stays enabled, redirects are rejected, only fixed GitHub URL templates are used, network reads are capped at 4 MB, timeouts are bounded, and images must successfully decode within 4096×4096 dimensions. Invalid or corrupt cache images are not displayed; a later scan replaces corrupt files.
