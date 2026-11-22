# Design Language & UI System

This document outlines the visual design system, typography tokens, layout specifications, and rendering implementations for Owlmic across Android (Jetpack Compose) and Windows (Native Win32 GDI/GDI+).

The definitive design tokens and strings are defined in [`shared/design/tokens.json`](../../shared/design/tokens.json) and [`shared/design/copy.json`](../../shared/design/copy.json).

---

## 1. Design Philosophy

Owlmic adheres to a strict **pure functional** design aesthetic:
- **Zero AI Slop / Visual Bloat**: No decorative gradients, no drop shadows, no glassmorphism / translucent blur, and no springy physics animations.
- **OLED Pure Black**: Absolute black (`#000000`) canvas on mobile to conserve battery and eliminate backlight bleed.
- **Hairline Precision**: 1-pixel crisp hairlines (`#262626`) define tiles and containers.
- **High-Contrast Signal States**: Color is used exclusively for actionable state:
  - Inactive / Disabled: `#3A3A3C`
  - Active / ON: High-contrast white (`#FFFFFF`) with inverted black glyphs.
  - Live Connection / Healthy: `#30D158` (Green)
  - Waiting / Warning: `#FFD60A` (Yellow)
  - Disconnected / Error: `#FF453A` (Red)

---

## 2. Design Tokens (`shared/design/tokens.json`)

### Color Palette
| Token | Hex Value | Purpose |
| :--- | :---: | :--- |
| `color.bg` | `#000000` | Fullscreen OLED black background |
| `color.tile` | `#0A0A0A` | Tile and card surface background |
| `color.hairline` | `#262626` | 1-pixel crisp border outlines |
| `color.text` | `#FFFFFF` | Primary high-contrast text |
| `color.text2` | `#8E8E93` | Secondary text, captions, and metrics |
| `color.disabled` | `#3A3A3C` | Disabled state or inactive indicator |
| `color.green` | `#30D158` | Connected, OK, healthy link (PC panel only) |
| `color.yellow` | `#FFD60A` | Waiting for approval, reconnecting (PC panel only) |
| `color.red` | `#FF453A` | Disconnected, error, blocked (PC panel only) |
| `color.gap` | `#7E7E7E` | The 1 dp seam between phone tiles |
| `color.ink` | `#E6E6E6` | Phone icons, the sound wave, sliders and switches; a top tile whose drawer is open (with a black icon) |
| `color.pressed` | `#1A1A1A` | A tile while pressed |
| `color.rule` | `#1C1C1E` | Row dividers in the phone drawers |
| `color.chip`, `color.chipOn` | `#141416`, `#151518` | Buttons on the phone pages; the selected choice in settings |
| `color.chipOnEdge`, `color.chipOffEdge` | `#3E3E44`, `#1A1A1D` | Edges of link buttons that are on and off |
| `color.faint` | `#555558` | Links switched off, the version on the info page |

### Typography Scale
| Style | Phone (sp / line) | PC (px / line) | Weight | Use Case |
| :--- | :---: | :---: | :---: | :--- |
| **Display** | 28 / 34 | 24 / 30 | 600 (SemiBold) | Feature headers, prominent codes |
| **Title** | 17 / 22 | 14 / 18 | 600 (SemiBold) | Tile titles, modal headings |
| **Body** | 15 / 20 | 13 / 17 | 400 (Regular) | Primary labels, button text |
| **Caption** | 12 / 16 | 11 / 14 | 400 (Regular) | Status copy, codecs, framerates |

The phone's pages over the preview (connection, info, settings) and its controls use a size up, read at arm's length: names 17 sp SemiBold, text 16 sp, details 14 sp, section labels 13 sp SemiBold (`Theme.pageName`, `pageText`, `pageDetail`, `pageLabel`).

The phone uses no colour: green, yellow and red are the PC panel's. On the phone, state reads from the icon (the link tile's globe or alert) and what is on or selected carries a tick.

### Spatial System & Geometry
- **Base Grid Unit**: 4 px / 4 dp.
- **Hairlines**: Exactly 1 px at all display DPI scalings.
- **Corner Radii**:
  - Android main screen tiles: 12 dp (`phone.tileRadiusDp`), on a 1 dp seam (`phone.gapDp`).
  - Windows Flyout Panel: 16 px (`radius.panelPx`).

---

## 3. Windows Native Flyout Panel (`desktop/crates/owlmic-ui`)

Rather than shipping Chromium/Electron or bloated webview engines, Owlmic PC uses a **100% native Win32 GDI/GDI+ renderer**:
- **Geometry**: Bento 6-Square layout with a fixed 312 × 374 px panel positioned above the taskbar system tray.
- **Double-Buffered Blitting**: Drawn entirely off-screen to a memory device context (DC) and blitted to screen in a single `BitBlt` call to guarantee zero flicker.
- **Instant Response**: Panel opens in < 15 ms with zero cold-start delay.
- **Live Video Preview & Playground**: A 310 × 310 px square playground tile displays live camera video preview, waveform dock, and overlay drawers directly inside the flyout, with 5 square action buttons across the bottom row.

---

## 4. Android Compose UI (`android/app`)

The main screen strictly replicates the Bento layout prototyped in `shared/design/prototypes/`: OLED black tiles on a 1 dp grey seam (`phone.gapDp`), organized into three primary functional bands.

| Band | Geometry | Contents |
| :--- | :--- | :--- |
| **Top Bar** | 66 dp height (`phone.topBarDp`) | OWLMIC wordmark, **?** tile that opens Help & Info, and connection tile |
| **Playground Area** | Flexible weight (`1f`) | Centered owl watermark on dot grid when idle; live camera preview with optical crop borders and expandable format HUD; docked/floating audio waveform; status strip |
| **Bottom Bar** | 124 dp height (`phone.bottomBarDp`) | 3 square Bento feature tiles (Camera, Mic, Speaker) with 35 dp glyphs; tap to toggle ON/OFF, long-press for feature settings sheet |

### Key Screen Invariants & Mechanics
- **Text & Status Signals**: Minimalist high-signal copy. The status strip shows PC state, manual selection, or user actions (tethering switch, update prompt). Searching, connecting, and reconnecting remain silent, conveyed visually by the pulsing connection tile.
- **Top-Bar Tiles & Glyphs**:
  - Wordmark dynamically rotates letter-by-letter to match device orientation.
  - Help and Connection tiles use 26 dp glyphs (`phone.topGlyphDp`).
  - Connection tile displays active link type (USB, Tethering, Wi-Fi, Bluetooth), globe during discovery, or alert glyph for attention. During negotiation, the icon breathes smoothly (`linkPulseLow`, `linkPulseMs`).
- **Feature Tiles**:
  - High-contrast 35 dp vector glyphs (`phone.glyphDp`) centered in 124 dp square tiles.
  - Inverted pure white (`#FFFFFF`) with black glyphs when active; muted outline when off; dimmed when no PC is connected.
- **Drawers**:
  - Slide down over the playground area with cubic easing (`DRAWER_SLIDE_MS`, `DRAWER_FADE_MS`).
  - **Connection Drawer**: Zero-dialog inline handshake sweep (`sync-beam` animated gradient line on active connecting PC row). Inline `+` / `Add PC` manual host entry. 6-column transport footer: Refresh button (spin animation), 1 px vertical divider, and 4 transport toggle buttons (USB debugging, Tethering, Wi-Fi, Bluetooth).
  - **Help & Info Drawer**: 2×3 Bento grid (How-to, Privacy, Rate, GitHub, Donate, Credits) with dedicated subpages and back navigation for How-to, Privacy, and Credits.
- **Camera Preview & Formatting**:
  - **Auto Zoom-to-Fit**: Camera preview rendered via `GlRenderer` uses cover scaling to fill the available playground area without letterbox bars.
  - **Optical Crop Borders**: Crisp hairline guides outline target aspect ratios when explicit framing is chosen.
  - **Format Heads-Up Display (HUD)**: Compact expandable dropdown menu at top-left (closed: 50 × 29 dp, open: 180 × 128 dp) featuring dual-axis easing (`CubicBezierEasing(0.16f, 1f, 0.3f, 1f)`) and Crossfade transitions between Resolution (FHD 1080p, HD 720p, 480p), Framerate (60fps, 30fps, 24fps), and Aspect Ratio (Auto, 16:9, 4:3, 1:1, 3:4, 9:16).
  - **HUD Auto-Dimming**: Format menu and top-right mirror/flip buttons fade to 0.15 alpha after 4 seconds idle (`controlsIdleMs`), waking immediately upon any screen touch.
- **Audio Waveform & Controls**:
  - Centered 208 dp container for waveform bars; docks into a bottom capsule when camera is active.
  - Tapping the docked capsule expands it upward into audio controls (voice isolation toggle, auto-level switch, noise reduction slider, volume slider) without stretching the internal waveform bars horizontally.
- **Dynamic Glyph Pipeline**:
  - All icons are dynamically parsed from `shared/assets/icons/*.svg` by the Gradle task `:core:generateDesign` into `GeneratedGlyphs.kt`.
  - `Glyphs.kt` provides a clean 1:1 type-safe enum mapping with zero hardcoded path strings or legacy aliases.
- **Launcher Icon**:
  - Pure OLED black background (`#000000`) with centered official owl vector scaled to 0.54 (`ic_launcher_foreground.xml`), providing ~3.25 dp breathing room padding per side.
- **Sideways & Orientation**:
  - Activity remains portrait-locked; UI responds to rotation angles (0°, 90°, 270°) by fading content, rotating icon glyphs, and reflowing drawer layouts smoothly.
- **Single Activity Architecture**:
  - `MainActivity.kt` observes immutable state from `AppHub`; deterministic UI decisions are calculated in `ScreenModel.kt` and tested in `ScreenModelTest.kt`.
