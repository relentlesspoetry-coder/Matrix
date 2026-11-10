# Matrix-Play artwork

The original logo is stored in `web/assets/matrix-play-logo.png`. The sidebar uses the full image; the compact sidebar and desktop launcher use its central symbol.

To rebuild the icons:

```bash
npm ci
python3 scripts/prepare-icons.py
```

The script crops the original artwork and generates the required PNG sizes. Application IDs, settings paths and game IDs remain unchanged.

## Console artwork

Original console images are in `branding/console-originals/`. Their library IDs are mapped in `branding/consoles.json`; runtime images are in `web/assets/consoles/`.

The [MIT license](LICENSE) covers source code. Third-party console artwork, trademarks and game covers retain their own rights.
