# Web Landing & Release Site - Domain Skill

> **Scope**: Code living in `website/` (React, TypeScript, Vite, web styling, and documentation landing).

---

## 1. Tech Stack & Architecture

- **Core Framework**: React 19 + TypeScript + Vite.
- **Styling**: Pure CSS / modules strictly derived from `shared/design/tokens.json`.
- **Assets & Icons**:
  - Master brand marks from `shared/assets/brand/` (`icon.svg`, `icon-adaptive.svg`, `icon-badge.svg`).
  - Glyphs and status icons from `shared/assets/icons/`.
- **Copy & Invariants**:
  - Headings, taglines, and feature descriptions must reflect `shared/design/copy.json`.
  - Product name casing invariant: strictly `Owlmic` or `owlmic`.

---

## 2. Invariants & Web Guidelines

1. **Zero Telemetry & Absolute Privacy**:
   - Never embed Google Analytics, Mixpanel, Segment, Meta Pixel, or third-party tracking scripts.
   - Zero cookies. The site must not require cookie consent banners.
2. **Minimal Bundle & Zero Framework Slop**:
   - Do not install heavy component libraries (e.g., MUI, Ant Design, Chakra UI, Tailwind runtime).
   - Keep dependencies lightweight and modern.
3. **Design System Alignment**:
   - OLED black theme: `#000000` true black background, `#0A0A0A` surface tiles, `#262626` borders/dividers.
   - Accent states: `#30D158` (active/success), `#FFD60A` (connecting/warning), `#FF453A` (error).
   - Typography: Clean monospace and sans-serif pairings matching Owlmic brand identity.
4. **Download Links & Release Routing**:
   - Download buttons route users directly to the latest GitHub release assets or the local `dist/` outputs (`Owlmic-Setup-{version}.exe`, `Owlmic-v{version}.apk`, `Owlmic-v{version}-linux-x86_64.tar.gz`).

---

## 3. Verification Commands

Run before proposing changes to `website/`:

```bash
cd website
npm run lint
npm run build
npm run format:check
```
