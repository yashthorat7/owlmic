# Design Language & UI System

This document outlines the visual design system, typography tokens, layout specifications, and rendering implementations for Owlmic across Android (Jetpack Compose) and Windows (Native Win32 GDI/GDI+).

The definitive design tokens and strings are defined in [`design/tokens.json`](../../design/tokens.json) and [`design/copy.json`](../../design/copy.json).

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

## 2. Design Tokens (`design/tokens.json`)

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
  - Windows Flyout Panel: 12 px (`radius.panelPx`).

---

## 3. Windows Native Flyout Panel (`pc/crates/owlmic-ui`)

Rather than shipping Chromium/Electron or bloated webview engines, Owlmic PC uses a **100% native Win32 GDI/GDI+ renderer**:
- **Geometry**: Fixed 560 × 255 px panel positioned above the taskbar system tray.
- **Double-Buffered Blitting**: Drawn entirely off-screen to a memory device context (DC) and blitted to screen in a single `BitBlt` call to guarantee zero flicker.
- **Instant Response**: Panel opens in < 15 ms with zero cold-start delay.
- **Live Video Preview**: A 352 × 198 px camera tile displays a 10 FPS NV12-to-RGB rendered video preview directly inside the flyout.

---

## 4. Android Compose UI (`android/app`)

The main screen follows the prototype in `landing/proto-templates`: black tiles on a 1 dp grey seam, in three bands.

| Band | Contents |
| :--- | :--- |
| Top row (72 dp) | The OWLMIC wordmark, a **?** tile that opens Setup & Help, and the link tile that opens Connection |
| Preview area | The owl on a dot grid while nothing is on; the mic's sound wave; the camera's picture with its capsule while it is on; the status strip while not connected |
| Bottom row (118 dp) | Camera, mic and speaker icon tiles: tap to turn on or off, long-press for that feature's settings |

- **Text:** only what the user must read or act on. The status strip shows the PC's name and the approval code, or a step only the user can take (choose a PC, ask again, update, forget a changed PC, turn on USB tethering, try Bluetooth). Looking, connecting and reconnecting say nothing; the link tile shows them. No button carries text, except the info page's six.
- **Link tile:** always in ink. Its icon is the link in use (USB debugging, USB tethering, Wi-Fi, Bluetooth) once connected; a globe until then; an alert when something needs the user (refused, busy, update, key changed, can't reach). While on its way the icon dims softly in and out (`linkPulseLow`, `linkPulseMs`): no blink, no glow.
- **Feature tiles** show only their icon, slashed while off, and are dimmed until a PC is connected. Why one is paused, restarting or stuck is said to screen readers.
- **Drawers** slide down from the top bar, from the tile that opened it, and fill the preview area. Content only, no header: the same tile or Back closes it.
  - **Connection:** the connected PC first with a tick, then the other PCs this phone used (most recent first), new PCs that answered and addresses added by hand; blocked PCs below a line. A row is only the name; it opens in place into two rows: the address with the link it used as a quiet icon, then when it was last connected with Connect, Block, Unblock or Remove. The footer is five equal buttons: Reconnect, a line like the one above, then USB debugging, Wi-Fi, USB tethering and Bluetooth (bright when on, faint when off).
  - **Info:** two rows of three square buttons, How-to, Privacy Policy, Rate, GitHub, Donate and Credits, and the version at the very bottom. How-to (the three steps as numbered circles on a line) and Credits open as pages of their own with only a back arrow.
- **Controls** (`Controls.kt`): ink on black. A setting with steps is a slider whose first step is off (noise reduction) or whose middle is neutral (volume, 0 dB); on/off settings are switches with a tick on the thumb. No chevrons and no glow anywhere on the preview.
  - **Mic only:** the sound wave sits in the middle of the preview, no wider than the capsule would be. A round settings button at the bottom right opens the mic's settings from the top, ending in a line, with the wave under it.
  - **Camera on:** small round buttons at the top right: eye (the preview off and back on; the PC still gets the picture), mirror preview and flip, light when on. With the mic on too, the wave sits in a capsule at the bottom, 80% of the phone's width upright and the same width sideways (`capsuleWidthFraction`, `capsuleHeightDp`); tapping it grows it upward into the mic's settings, ending in a line above the wave.
  - The round buttons rest at 20% (`controlsDimAlpha`) and work there; a touch anywhere on the preview brings them up over 200 ms (`controlsFadeMs`), and 3 s without one (`controlsIdleMs`) lets them back down.
  - The preview is what the PC gets, fitted whole and upright for whoever holds the phone. Only when fitting leaves room, a blurred copy of the same picture fills it. A black line as wide as the grid's seam frames it.
- **Sideways:** the activity stays locked to portrait and the layout never moves. One quarter-turn value (0, 90 or 270, from an `OrientationEventListener` with 20 degrees of hysteresis past the diagonal; upside down and flat keep the last value) drives a change in three steps: what is on the preview (and the wordmark, drawers and settings) fades out (`turnFadeMs`), the tile icons turn over 250 ms (`turnMs`), then the content fades back in laid out for the new hold. The status bar hides or comes back at that same moment. The wordmark letters turn one by one, a little apart (`wordmarkLetterGapDp`), and always read O to C top to bottom. Drawers and the settings sheet are laid out for the turned shape, then turned, so rows span the reader's width. A black band (`sideBandDp`) sits under the bottom tiles either way, and over the top tiles while sideways.
- Single activity (`MainActivity.kt`) observing immutable state from `AppHub`; screen decisions live in `ScreenModel.kt` and are unit tested, the turn in `Turn.kt`. Icons are drawn in `Glyphs.kt`; the wordmark and owl are vector drawables.
