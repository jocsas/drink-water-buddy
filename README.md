# Hydrate Buddy

Hydrate Buddy is a tiny Electron companion that lives in the macOS menu bar and
nudges you to drink water without sitting in the Dock. It pops in from the
corner, says something charming, reacts when you drink or snooze, and then gets
out of the way.

It is built as a playful desktop pet rather than a productivity dashboard: small
surface area, local settings, pixel art, themed messages, and just enough
configuration to make the reminders feel personal.

## Highlights

- Menu bar only on macOS, with no Dock icon.
- Reminder popup that appears without stealing keyboard focus.
- Configurable name, reminder interval, snooze duration, and visual theme.
- Per-user settings stored in the OS user-data folder.
- Sleep/wake-aware scheduler so reminders recover after the laptop resumes.
- Theme-specific colors, copy, tray icons, and character sprites.
- Animated sprite support for richer themes such as `Wizard`.
- Local/friends build scripts plus stricter signed release scripts.

## Themes

Hydrate Buddy currently ships with four themes:

- `Default doll`: the original cozy hydration buddy.
- `Baby Yoda`: soft greens, tiny-sage copy, and fan-style water nudges.
- `Darth Vader`: dark red UI with dramatic, funny hydration commands.
- `Wizard`: animated walking and drinking frames cut from a sprite sheet.

Theme assets use the same folder shape so new characters are easy to add:

```text
assets/themes/<theme>/
  idle.png
  drinking.png
  tray.png
```

`tray.png` is optional. If it is missing, Hydrate Buddy falls back to the default
tray icon. If `drinking.png` is missing, the app can fall back to `idle.png`.

Animated themes can also provide frame sequences:

```text
assets/themes/wizard/frames/
  walk-1.png
  walk-2.png
  walk-3.png
  walk-4.png
  drink-1.png
  drink-2.png
  happy.png
  cast.png
```

The Wizard theme is generated from a sprite sheet with:

```bash
python3 scripts/extract-wizard-theme.py /path/to/wizard-sprite-sheet.png
```

The final `Bye`/warp frame from the source sheet is intentionally not used.

## Development

Install dependencies once:

```bash
npm install
```

Run normally:

```bash
npm start
```

Run with automatic Electron restart while editing:

```bash
npm run dev
```

The dev runner watches `main.js`, `preload.js`, `renderer/`, `assets/`, and
`package.json`.

## Building

For a local macOS test build without distribution signing:

```bash
npm run dist:mac:dev
```

For a direct Apple Silicon build to share with trusted friends:

```bash
npm run dist:mac:friends
```

That produces:

```text
dist/HydrateBuddy-0.1.0-arm64.dmg
```

For a universal Intel plus Apple Silicon friend build:

```bash
npm run dist:mac:friends:universal
```

Friend builds are ad-hoc signed and not notarized, so macOS may show a first-run
warning. Public release builds should use a Developer ID Application certificate
and Apple notarization.

For a signed/notarized macOS release:

```bash
npm run dist:mac
```

For a draft GitHub release:

```bash
export GH_TOKEN=ghp_your_token
npm run release:github
```

The release scripts intentionally block unsafe public builds when the required
Developer ID and notarization credentials are missing.

## Project Layout

```text
main.js                     Electron app, tray menu, scheduler, settings
preload.js                  Safe IPC bridge for the renderer
renderer/                   Popup UI, settings UI, animations
shared/themes.js            Theme registry, colors, copy, animation metadata
assets/themes/              Character sprites and tray icons by theme
scripts/                    Release checks and asset extraction helpers
build/                      macOS entitlements and app icon
```

## Asset Notes

Hydrate Buddy supports fan-style and custom themes, but distributed builds should
only include assets you have the right to share. The app code is MIT licensed;
individual image assets may have their own usage terms.
