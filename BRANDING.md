# Matrix Play artwork

The user supplied the product name **Matrix Play** and the original PNG. The complete original is stored unchanged as `web/assets/matrix-play-logo.png` (1448×1086 RGB; SHA-256 `26f3d9dec4190dd26019e0269e42e927516af9d49d00fb15db2bd026451fe6bb`).

The desktop sidebar displays the full artwork. The compact sidebar and launcher icons use the artwork's central M/play symbol. `scripts/prepare-icons.py` wraps that region in an SVG crop, centers it on a dark square canvas, and uses the pinned Tauri CLI to produce 32, 128 and 512 pixel PNG icons. It does not redraw or regenerate the logo.

To reproduce the icons, run `npm ci` and `python3 scripts/prepare-icons.py` from the repository root. Brand display changes do not change the existing application identifier, persisted settings/cover paths, or game IDs.

## Console artwork

The user supplied 18 PNG files in Consoles.zip. Unchanged originals are stored in `branding/console-originals/`. `branding/consoles.json` maps 14 images to the existing console library IDs. The runtime WebP copies in `web/assets/consoles/` retain the full image and wordmark, fit within 1024×768, and use quality 88. Total runtime images are about 946 KiB.

PS4, PS5, PlayStation Vita and Switch 2 originals are retained but not displayed as libraries; their artwork does not imply implementation. Product and console artwork is supplied branding; the source-code MIT license does not grant rights to third-party console trademarks or game covers.
