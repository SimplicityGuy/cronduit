# Cronduit Design System

> **cron, modernized for the container era**

This document defines the complete visual identity and design language for Cronduit, a Docker-native cron scheduling system. Use this as a reference when building UI, documentation, marketing materials, or any visual asset for the project.

---

## 1. Brand Identity

### Tagline
```
cron, modernized for the container era
```

### Voice & Tone
- Developer-first, terminal-native aesthetic
- Retro-terminal warmth meets modern precision
- Confident but not flashy — tools should feel reliable
- Monospace everything — the typewriter is the brand

### Logo Variants
| Variant | File | Usage |
|---|---|---|
| Banner (animated) | `banners/cronduit-banner-animated.svg` | README hero, landing page |
| Banner (static) | `banners/cronduit-banner-static.svg` | GitHub social preview, print |
| Square logo (dark) | `logos/cronduit-square-dark.svg` | App icon, avatars, dark backgrounds |
| Square logo (light) | `logos/cronduit-square-light.svg` | Light backgrounds, print |
| Favicon 16×16 | `favicons/favicon-16.svg` | Browser tab |
| Favicon 32×32 | `favicons/favicon-32.svg` | Browser tab (HiDPI) |
| Favicon 192×192 | `favicons/favicon-192.svg` | Android home screen |
| Favicon 512×512 | `favicons/favicon-512.svg` | PWA splash, app stores |

---

## 2. Color System

All colors are designed to work in both dark and light mode. The palette is built around the signature terminal green, with status colors tuned to the same muted, slightly desaturated tone.

### 2.1 Brand Colors

| Token | Dark Mode | Light Mode | Usage |
|---|---|---|---|
| `--cd-green` | `#34d399` | `#059669` | Primary brand, active/success state |
| `--cd-green-dim` | `rgba(52, 211, 153, 0.15)` | `rgba(5, 150, 105, 0.1)` | Green backgrounds, highlights |
| `--cd-green-bright` | `#6ee7b7` | `#34d399` | Hover states, emphasis |

### 2.2 Status Colors

All status colors share the same saturation range (~55-70%) and lightness range (~55-65% in dark mode) to feel cohesive.

| Token | Dark Mode | Light Mode | Semantic | Usage |
|---|---|---|---|---|
| `--cd-status-active` | `#34d399` | `#059669` | Active/Success | Running successfully, healthy |
| `--cd-status-running` | `#60a5fa` | `#2563eb` | Running/In-Progress | Job currently executing |
| `--cd-status-disabled` | `#fbbf24` | `#d97706` | Disabled/Warning | Paused jobs, warnings |
| `--cd-status-error` | `#f87171` | `#dc2626` | Error/Failed | Failed jobs, errors |
| `--cd-status-stopped` | `#94a3b8` | `#64748b` | Operator-Interrupt | Jobs stopped via UI; NOT a failure |

#### Status Background Tints (for badges, pills, table rows)

| Token | Dark Mode | Light Mode |
|---|---|---|
| `--cd-status-active-bg` | `rgba(52, 211, 153, 0.12)` | `rgba(5, 150, 105, 0.08)` |
| `--cd-status-running-bg` | `rgba(96, 165, 250, 0.12)` | `rgba(37, 99, 235, 0.08)` |
| `--cd-status-disabled-bg` | `rgba(251, 191, 36, 0.12)` | `rgba(217, 119, 6, 0.08)` |
| `--cd-status-error-bg` | `rgba(248, 113, 113, 0.12)` | `rgba(220, 38, 38, 0.08)` |
| `--cd-status-stopped-bg` | `rgba(148, 163, 184, 0.12)` | `rgba(100, 116, 139, 0.08)` |

### 2.3 Surface & Background Colors

| Token | Dark Mode | Light Mode | Usage |
|---|---|---|---|
| `--cd-bg-primary` | `#050508` | `#f8f8f6` | Page background |
| `--cd-bg-surface` | `#0a0d0b` | `#ffffff` | Cards, panels |
| `--cd-bg-surface-raised` | `#0f1512` | `#f0f0ed` | Elevated surfaces, modals |
| `--cd-bg-surface-sunken` | `#030405` | `#e8e8e4` | Inset areas, code blocks |
| `--cd-bg-hover` | `#141a16` | `#e8ebe9` | Hover state on surfaces |

### 2.4 Border Colors

| Token | Dark Mode | Light Mode | Usage |
|---|---|---|---|
| `--cd-border` | `#1e2a22` | `#d4d8d5` | Default borders |
| `--cd-border-subtle` | `#141a16` | `#e0e4e1` | Subtle dividers |
| `--cd-border-strong` | `#2a3a2f` | `#b0b8b3` | Emphasized borders |
| `--cd-border-focus` | `#34d399` | `#059669` | Focus rings |

### 2.5 Text Colors

| Token | Dark Mode | Light Mode | Usage |
|---|---|---|---|
| `--cd-text-primary` | `#d0ddd4` | `#1a1f1c` | Primary body text |
| `--cd-text-secondary` | `#7a8a7e` | `#5a6a5e` | Secondary, descriptions |
| `--cd-text-muted` | `#4a5a4e` | `#8a9a8e` | Disabled, placeholders |
| `--cd-text-inverse` | `#0a0d0b` | `#f8f8f6` | Text on colored backgrounds |
| `--cd-text-accent` | `#34d399` | `#059669` | Links, accented text |
| `--cd-text-code` | `#d0ddd4` | `#1a1f1c` | Inline code text |

### 2.6 Terminal Chrome Colors

Used for terminal-style UI elements (the logo frame, code blocks, etc.)

| Token | Dark Mode | Light Mode | Usage |
|---|---|---|---|
| `--cd-terminal-bg` | `#0a0d0b` | `#1a1f1c` | Terminal background (always dark in light mode) |
| `--cd-terminal-chrome` | `#0f1512` | `#252b27` | Terminal title bar |
| `--cd-terminal-dot-red` | `rgba(255, 95, 87, 0.6)` | `rgba(255, 95, 87, 0.6)` | Window dot |
| `--cd-terminal-dot-yellow` | `rgba(254, 188, 46, 0.6)` | `rgba(254, 188, 46, 0.6)` | Window dot |
| `--cd-terminal-dot-green` | `rgba(40, 200, 64, 0.6)` | `rgba(40, 200, 64, 0.6)` | Window dot |
| `--cd-terminal-prompt` | `rgba(52, 211, 153, 0.5)` | `rgba(52, 211, 153, 0.5)` | `~$` prompt color |

> **Note:** Terminal-framed elements (logo, code blocks) always use a dark background even in light mode. This preserves the terminal identity.

---

## 3. Typography

### Font Family

**Primary (all UI):** `JetBrains Mono`
- Available on Google Fonts: `https://fonts.google.com/specimen/JetBrains+Mono`
- CSS import: `@import url('https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;500;700&display=swap');`

**Fallback stack:**
```css
font-family: 'JetBrains Mono', 'Fira Code', 'Source Code Pro', 'Cascadia Code', 'Consolas', monospace;
```

> Cronduit uses a single monospace font for ALL text — headings, body, labels, code. This is intentional and core to the brand identity. Never mix in a sans-serif or serif font.

### Font Weights

| Weight | Token | CSS | Usage |
|---|---|---|---|
| Regular | `--cd-font-regular` | `font-weight: 400` | Body text, descriptions, labels |
| Medium | `--cd-font-medium` | `font-weight: 500` | Emphasized body, nav items |
| Bold | `--cd-font-bold` | `font-weight: 700` | Headings, brand name, buttons |

### Font Sizes

| Token | Size | Line Height | Usage |
|---|---|---|---|
| `--cd-text-xs` | `0.65rem` (10.4px) | 1.5 | Captions, badges, micro labels |
| `--cd-text-sm` | `0.8rem` (12.8px) | 1.5 | Secondary text, metadata |
| `--cd-text-base` | `0.9rem` (14.4px) | 1.6 | Body text, form inputs |
| `--cd-text-md` | `1rem` (16px) | 1.5 | Emphasized body |
| `--cd-text-lg` | `1.25rem` (20px) | 1.4 | Section headings |
| `--cd-text-xl` | `1.5rem` (24px) | 1.3 | Page titles |
| `--cd-text-2xl` | `2rem` (32px) | 1.2 | Hero text |

### Letter Spacing

| Context | Value |
|---|---|
| Body text | `0` (default) |
| Headings | `-0.02em` |
| Labels / uppercase | `0.1em` — `0.2em` |
| Brand wordmark | `0.05em` |

---

## 4. Spacing & Layout

### Spacing Scale

Based on a 4px grid:

| Token | Value |
|---|---|
| `--cd-space-1` | `4px` |
| `--cd-space-2` | `8px` |
| `--cd-space-3` | `12px` |
| `--cd-space-4` | `16px` |
| `--cd-space-5` | `20px` |
| `--cd-space-6` | `24px` |
| `--cd-space-8` | `32px` |
| `--cd-space-10` | `40px` |
| `--cd-space-12` | `48px` |
| `--cd-space-16` | `64px` |

### Border Radius

| Token | Value | Usage |
|---|---|---|
| `--cd-radius-sm` | `4px` | Badges, small pills |
| `--cd-radius-md` | `8px` | Buttons, inputs, cards |
| `--cd-radius-lg` | `12px` | Panels, modals |
| `--cd-radius-xl` | `16px` | Large cards, containers |
| `--cd-radius-full` | `9999px` | Circles, fully rounded pills |

---

## 5. Component Patterns

### 5.1 Status Badges

```html
<span class="cd-badge cd-badge--active">Active</span>
<span class="cd-badge cd-badge--running">Running</span>
<span class="cd-badge cd-badge--disabled">Disabled</span>
<span class="cd-badge cd-badge--error">Error</span>
```

**Styling:**
- Background: respective `--cd-status-*-bg`
- Text: respective `--cd-status-*`
- Font: `--cd-text-xs`, `--cd-font-bold`, uppercase, `letter-spacing: 0.1em`
- Padding: `2px 8px`
- Border-radius: `--cd-radius-sm`

### 5.2 Buttons

**Primary button:**
- Background: `--cd-green`
- Text: `--cd-text-inverse`
- Font: `--cd-text-sm`, `--cd-font-bold`
- Padding: `8px 16px`
- Border-radius: `--cd-radius-md`
- Hover: `--cd-green-bright`

**Secondary button:**
- Background: transparent
- Border: `1px solid --cd-border`
- Text: `--cd-text-primary`
- Hover background: `--cd-bg-hover`

**Danger button:**
- Background: `--cd-status-error`
- Text: `--cd-text-inverse`

### 5.3 Cards / Panels

- Background: `--cd-bg-surface`
- Border: `1px solid --cd-border`
- Border-radius: `--cd-radius-lg`
- Padding: `--cd-space-6`

### 5.4 Code Blocks / Terminal Frames

Always render with dark terminal styling, even in light mode:
- Background: `--cd-terminal-bg`
- Border: `1px solid --cd-border`
- Include terminal chrome (three dots) for hero/decorative usage
- Prompt symbol: `~$` in `--cd-terminal-prompt`
- Text: `--cd-text-primary` (dark mode values, always)
- Border-radius: `--cd-radius-md`

### 5.5 Tables

- Header: `--cd-bg-surface-raised`, `--cd-font-bold`, `--cd-text-xs`, uppercase
- Rows: `--cd-bg-surface`, alternate with `--cd-bg-surface-sunken` (optional)
- Row hover: `--cd-bg-hover`
- Borders: `--cd-border-subtle` horizontal only

### 5.6 Form Inputs

- Background: `--cd-bg-surface-sunken`
- Border: `1px solid --cd-border`
- Text: `--cd-text-primary`
- Placeholder: `--cd-text-muted`
- Focus: `--cd-border-focus` with `0 0 0 2px --cd-green-dim` box-shadow
- Border-radius: `--cd-radius-md`
- Font: `--cd-text-base`, `--cd-font-regular`

---

## 6. Dark / Light Mode Implementation

### CSS Custom Properties Approach

```css
:root,
[data-theme="dark"] {
  --cd-green: #34d399;
  --cd-green-dim: rgba(52, 211, 153, 0.15);
  --cd-green-bright: #6ee7b7;

  --cd-status-active: #34d399;
  --cd-status-running: #60a5fa;
  --cd-status-disabled: #fbbf24;
  --cd-status-error: #f87171;
  --cd-status-stopped: #94a3b8;

  --cd-status-active-bg: rgba(52, 211, 153, 0.12);
  --cd-status-running-bg: rgba(96, 165, 250, 0.12);
  --cd-status-disabled-bg: rgba(251, 191, 36, 0.12);
  --cd-status-error-bg: rgba(248, 113, 113, 0.12);
  --cd-status-stopped-bg: rgba(148, 163, 184, 0.12);

  --cd-bg-primary: #050508;
  --cd-bg-surface: #0a0d0b;
  --cd-bg-surface-raised: #0f1512;
  --cd-bg-surface-sunken: #030405;
  --cd-bg-hover: #141a16;

  --cd-border: #1e2a22;
  --cd-border-subtle: #141a16;
  --cd-border-strong: #2a3a2f;
  --cd-border-focus: #34d399;

  --cd-text-primary: #d0ddd4;
  --cd-text-secondary: #7a8a7e;
  --cd-text-muted: #4a5a4e;
  --cd-text-inverse: #0a0d0b;
  --cd-text-accent: #34d399;
}

[data-theme="light"] {
  --cd-green: #059669;
  --cd-green-dim: rgba(5, 150, 105, 0.1);
  --cd-green-bright: #34d399;

  --cd-status-active: #059669;
  --cd-status-running: #2563eb;
  --cd-status-disabled: #d97706;
  --cd-status-error: #dc2626;
  --cd-status-stopped: #64748b;

  --cd-status-active-bg: rgba(5, 150, 105, 0.08);
  --cd-status-running-bg: rgba(37, 99, 235, 0.08);
  --cd-status-disabled-bg: rgba(217, 119, 6, 0.08);
  --cd-status-error-bg: rgba(220, 38, 38, 0.08);
  --cd-status-stopped-bg: rgba(100, 116, 139, 0.08);

  --cd-bg-primary: #f8f8f6;
  --cd-bg-surface: #ffffff;
  --cd-bg-surface-raised: #f0f0ed;
  --cd-bg-surface-sunken: #e8e8e4;
  --cd-bg-hover: #e8ebe9;

  --cd-border: #d4d8d5;
  --cd-border-subtle: #e0e4e1;
  --cd-border-strong: #b0b8b3;
  --cd-border-focus: #059669;

  --cd-text-primary: #1a1f1c;
  --cd-text-secondary: #5a6a5e;
  --cd-text-muted: #8a9a8e;
  --cd-text-inverse: #f8f8f6;
  --cd-text-accent: #059669;
}
```

### Auto Dark Mode (system preference)

```css
@media (prefers-color-scheme: light) {
  :root:not([data-theme="dark"]) {
    /* light mode values */
  }
}
```

---

## 7. Icon Guidelines

### Logo Mark (the clock-arrow icon)

The Cronduit logo mark consists of:
1. A circle (clock face) with two hands (hour pointing up, minute pointing right)
2. A horizontal line extending from the circle (conduit/pipeline)
3. A right-pointing arrow at the end (execution/flow)

**Proportions:**
- Circle radius: 7 units
- Clock hands: 1.5 stroke width
- Pipeline line: same stroke width, dashed or solid
- Arrow: equilateral triangle, 6 units tall

**Color:** Always `--cd-green` (or `--cd-status-active`)

**Clear space:** Maintain at least 1× the icon width of clear space around the mark.

### Sizing

| Context | Icon Size |
|---|---|
| Favicon | 12–14px mark within 16–512px canvas |
| Inline (text) | Match `font-size` of surrounding text |
| Nav / header | 24–32px |
| Hero | 48–64px |

---

## 8. File Manifest

```
cronduit-design-pack/
├── DESIGN_SYSTEM.md          ← This file
├── banners/
│   ├── cronduit-banner-static.svg
│   └── cronduit-banner-animated.svg
├── logos/
│   ├── cronduit-square-dark.svg
│   └── cronduit-square-light.svg
├── favicons/
│   ├── favicon-16.svg
│   ├── favicon-32.svg
│   ├── favicon-192.svg
│   └── favicon-512.svg
└── showcase.html             ← Visual reference (open in browser)
```

---

## 9. Quick Reference Card

```
FONT:     JetBrains Mono (400, 500, 700)
GREEN:    #34d399 (dark) / #059669 (light)
BLUE:     #60a5fa (dark) / #2563eb (light)
YELLOW:   #fbbf24 (dark) / #d97706 (light)
RED:      #f87171 (dark) / #dc2626 (light)
BG:       #050508 (dark) / #f8f8f6 (light)
SURFACE:  #0a0d0b (dark) / #ffffff (light)
TEXT:     #d0ddd4 (dark) / #1a1f1c (light)
BORDER:   #1e2a22 (dark) / #d4d8d5 (light)
RADIUS:   4 / 8 / 12 / 16px
GRID:     4px base
```
