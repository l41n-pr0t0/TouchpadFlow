# Design System: TouchpadFlow app

Visual language shared with MNEMOS-ZERO: a monochrome ink-on-paper print. Dark mode is the same sheet printed in negative.

## Dials
VARIANCE 4 · MOTION 1 · DENSITY 5. It is a settings tool: restraint executed strictly.

## Signature element
**The touchpad map**: a to-scale drawing of the pad. Edge zones are 45° halftone screentone, the boundaries are ink lines with a grip you drag to resize, the pointer area carries a drafting reticle, and the pad sits between corner crosshairs.

## Palette (exactly four tones)
| Token | Light | Dark | Role |
| :--- | :--- | :--- | :--- |
| ink | `#0c0c0b` | `#e6e6e2` | text, borders, active fills, offset shadows |
| ink-soft | `#4a4a48` | `#9b9b98` | captions, screentone dots |
| rule | `#9b9b98` | `#4a4a48` | hairlines, crosshairs, slider rails, hover |
| paper | `#e6e6e2` | `#0c0c0b` | background |

No hue anywhere. Emphasis comes from weight and inversion (paper on ink), never colour. Errors are shown as an inverted plate.

## Typography (bundled, OFL)
- Display: Space Mono Bold (title plate)
- Body and UI: IBM Plex Mono Regular; labels and buttons IBM Plex Mono SemiBold, uppercase

## Components
- 0px corners everywhere.
- Buttons: 2px ink border, 3px hard offset shadow; pressing sinks the button onto its shadow. "On" state is inverted.
- Section headers: `NN / TITLE` followed by a hairline rule.
- Status: solid square = running, hollow = paused/off, cross = not running.
- Sliders: thin rule rail, ink trailing fill, rectangular handle.

## Banned
Colour accents, gradients, glows, rounded corners, emojis, spinners.

## Themes
Picked under **06 / LOOK**: System, Light, Dark, Material. Light and Dark are the print above. **Background** turns the ink page off (print themes only).

Print themes sit on the **ink page**: a texture rendered once per window size (grain, halftone smoke in two corners, a drip cluster at top-right, hatching at lower-left, a firm border). It is fixed to the window and never scrolls.

### Material
Material 3 as implemented by Sung (github.com/yappologistic/Sung, `qml/Theme.qml`): its roles, shapes, type and state layers, with a **neutral seed**, so every role keeps its Material tone step but carries no hue. It follows the system light/dark setting: from the OS on Windows and macOS, and on Linux from the settings portal (`org.freedesktop.appearance color-scheme`, served by KDE and GNOME), re-read every 2 s and keeps Material's plain surface (the ink page is print only, so **Background** is disabled here).

| Role | Light | Dark | Used for |
| :--- | :--- | :--- | :--- |
| surface | `#f9f9f9` (N98) | `#131313` (N6) | page |
| surfaceContainer | `#eeeeee` (N94) | `#1f1f1f` (N12) | cards, idle buttons, inputs |
| onSurface | `#1b1b1b` (N10) | `#e2e2e2` (N90) | text, arrows |
| onSurfaceVariant | `#474747` (NV30) | `#c6c6c6` (NV80) | captions |
| outlineVariant | `#c6c6c6` (NV80) | `#474747` (NV30) | map outline, zone dividers, field borders |
| primary | `#3b3b3b` (25) | `#e2e2e2` (90) | selected buttons, section titles, slider fill, zone labels, grips |
| onPrimary | `#ffffff` | `#1b1b1b` | text on primary |
| primaryContainer | `#dcdcdc` (87) | `#3b3b3b` (25) | active zones, slider track |

No pure black or white surfaces; emphasis is tone contrast, never hue.

- **Type:** Google Sans Flex (OFL, bundled unmodified), sentence-case labels; section titles in primary, no numbers or rules.
- **Shape:** medium (12) for cards and the map, full pills for 40px buttons and labels.
- **States:** hover lays the content colour over the container at 8%, press at 10%; no borders or shadows on buttons; menus at elevation 2.
- **Slider:** 12px rounded tonal track, primary active fill, a thin bar handle.
