# 09 — Architectural Blueprint

## Thesis

Credit is a built thing. Not a feeling, not a flow, not a fluid — a structure. Bolted together from inspected components, stamped, dimensioned, signed off by a checker. CredMesh is rendered as a working drawing from a 1955 drafting room: blueprint emulsion paper, hairline white linework, hand-lettered notes in the margins where the engineer worked something out, printed callouts everywhere there's a load to name.

In this world, the dashboard is a sheet — sheet 01 of 12, scale 1:1, drawn by the protocol, checked by an oracle. A pool is a vessel with section dimensions. A reputation is a beam with a stamped rating. An advance is a connection detail: gusset plate, two bolts, leader line to a callout that names the part. There is no glow, no gradient, no illusion of motion. The page is a paper artifact and the numbers on it are stamped certifications. Capital, when treated this way, looks engineered the way a Carlo Scarpa joint looks engineered — every part is justified, every dimension is held, nothing is decorative.

The point is to make credit feel **certified**. The agents that borrow here, and the LPs that fund them, want a system whose load paths can be drawn. This is that drawing.

## Tagline

> EVERY FLOW HAS A LOAD PATH.

(Alt: *Credit, dimensioned.* / *Tolerances: ±0.0001 USDC.*)

## Design rules

1. Sheet color is blueprint cyan `#1B3A6F`. Linework is paper-cream `#F2EDE0`. Never pure white, never pure black.
2. All primary linework is hairline (0.75pt at full bleed). No thick borders, no heavy dividers. Weight is communicated by *layering*, not by stroke width.
3. Three typefaces only. **Barlow Condensed** ALL CAPS for printed labels (substitutes Trade Gothic Condensed). **Architects Daughter** for handwritten margin notes — never used for data. **JetBrains Mono** for every number.
4. Every numeric field gets a dimension line: witness tick at start, hairline rule, witness tick at end, value floating above the rule. Numbers do not appear without dimension lines.
5. No solid fills, ever. Filled zones are 45° hairline hatching at 8px spacing. This includes "selected" states, "active" highlights, "warning" zones.
6. No icons. Components are drawn as the *thing* they reference — a vessel for the pool, a beam for the reputation register, a fastener for an advance. If you can't draw it as an architectural element, it doesn't belong on the sheet.
7. Title block lives bottom-right, fixed dimensions. Includes: PROJECT, SHEET, SCALE, DATE, DRAWN, CHECKED, REV. The title block is sacred — never resized, never decorated.
8. Section markers (Ⓐ → Ⓑ) and registration crosses (+) appear at corners of every framed element. They are not optional; they are the grammar of the page.
9. Tables are *schedules*. Column heads in Barlow caps, row data in JetBrains Mono, hairline rules between columns and at top/bottom only. No row stripes, no zebra fills, no row hover.
10. The North Arrow points up — labeled `N: CONSENSUS`. It always exists, always in the same place. The metaphor is: the page has an orientation; capital flows have a direction.

## Anti-rules

- Never animate anything. The page is paper.
- Never use color to encode value. Color is paper or ink. Status is encoded in *callouts*.
- Never crop or hide a dimension line. If a number is on the page, its dimension is on the page.
- Never use a circle as a primary shape. The circle is reserved for bolt holes, registration marks, and section markers.
