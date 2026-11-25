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
