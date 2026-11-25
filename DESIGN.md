# Design System: TouchpadFlow app

## 0. Dials
VARIANCE: 3 | MOTION: 2 | DENSITY: 5. It is a settings tool, so restraint is the point.

## 1. Atmosphere
A quiet utility window. Zinc neutrals, one accent, no ornament. Follows the system light/dark theme.

## 1b. Signature element
**The touchpad map**: a to-scale drawing of the pad where the edge zones are shaded and you resize them by dragging the boundary lines. It is the product itself, not decoration. The same zone colour and labels appear in the column controls below it.

## 2. Palette
- **Zinc 900** (#18181B): dark panel background (never #000)
- **Zinc 50** (#FAFAFA): light panel background
- **Trackpad Teal** (#1F9E8A): the only accent. Active zones, sliders, "running" state
- **Brick** (#C24A3F): errors and "engine not running"
- **Ochre** (#B7862C): paused / gestures off

## 3. Typography
egui's bundled UI font and monospace; numbers in zones use monospace. No web fonts are loaded.

## 4. States
- Engine: checking / running / paused / gestures off / not running (Linux), "applies to platform engine" (macOS, Windows)
- Errors (invalid config, save failure) are shown inline in the footer, never as pop-ups
- A zone set to "Nothing" greys out and disables its sliders

## 7. Banned
Emojis, neon/purple glows, pure black, browser or localhost UI.
