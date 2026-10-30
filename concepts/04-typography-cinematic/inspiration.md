# Inspiration — references actually used

## Posters & cinematic typography

1. **Saul Bass — *Anatomy of a Murder* poster (1959)** — https://www.moma.org/collection/works/4824
   The fragmented black silhouette on saturated orange-red, hand-rendered type, asymmetric block composition. Took: rule #3 (one violent red mark per frame), and the idea that a single type-and-color combination can hold the weight of an entire identity. The "PENDING" tag in the recent-advances table is set in red as the only red word in that strip — the Bass move.

2. **Saul Bass — *Vertigo* / *Psycho* / *North by Northwest* title sequences** — http://artofthetitle.com/designer/saul-bass/
   Kinetic typography, type as protagonist, credits as a film-grammar device. Took: the "FRAME 01 / 47" counter, the colophon's "FIN." treatment, framing the recent-advances table as "A FILM IN FIVE FRAMES."

3. **Sebastian Pardo — *The Brutalist* (2024) title design** — https://grafismasakini.com/project-review/tracing-bauhaus-and-brutalism-in-the-title-design-of-the-brutalist/en
   Bold-faced type, dramatic spacing, type-only credit roll, the tension between minimalism and massivism. Took: the cinematic credit-roll structure of the recent-advances section; the discipline of using one heavy weight at extreme scale rather than hierarchical decoration.

## Identity & system design

4. **Massimo Vignelli — Bloomingdale's identity (1972)** — https://printmag.com/design-thinking/massimo-vignelli-creator-of-timeless-design-and-fearless-critic-of-junk
   The wordmark IS the identity; no glyph required. Vignelli stripped feet off the letters and made the curves the asset. Took: rule #1 directly. CredMesh has no logomark because Bloomingdale's didn't need one either.

5. **Massimo Vignelli — five-typeface discipline** — https://printmag.com/design-thinking/massimo-vignelli-creator-of-timeless-design-and-fearless-critic-of-junk
   "Helvetica is just like a piano, the more you play it, the more you learn how to play it." Took: rule #2 — three colors, one type family. Color discipline mirrors his typographic discipline.

6. **Vignelli NYC Subway Diagram (1972)** — https://nymag.com/homedesign/fall2007/39597/index1.html
   Every line a color, every stop a black dot, every angle 45 or 90. Took: the strict orthogonality of the dashboard grid (no diagonals, no curves); also the principle that abstract precision beats topographic realism — the dashboard prefers the *idea* of utilization (one big number) over a literal chart.

## Grid & programmed typography

7. **Wim Crouwel — *New Alphabet* (1967)** — https://collections.lacma.org/object/250419
   Letters reduced to horizontal/vertical strokes on a 5×7 grid; type designed for the constraints of the machine. Took: the geometric construction of the wordmark in `logo.svg` — each letter is built from `<rect>` elements on a unit grid. The protocol is for machines; the wordmark is composed from machine primitives.

8. **Wim Crouwel — Stedelijk posters / "Mr. Gridnik"** — https://cooperhewitt.org/2019/09/20/remembering-wim-crouwel-1928-2019
   The grid as a tool for generating flexible systems, not for suppressing creativity; visible underlying structure. Took: rule #9 — the 12-column hairline grid stays visible in the dashboard background, not stripped at handoff. Crouwel left the gridded paper visible in his prints; we leave the CSS `repeating-linear-gradient` visible in production.

9. **Josef Müller-Brockmann — Swiss grid systems** — referenced via Crouwel article above
   12-column grid as a positive design element, not invisible scaffolding. Took: the visible 1px hairline grid backdrop (Rule 9), and the discipline that section dividers are 1px solid ink rules rather than colored bars or shadows.

## Editorial discipline

10. **Hara Design Institute / MUJI manifestos**
    Tiny tracked text against monumental whitespace; small caps labels carrying enormous semantic weight; the dignity of restraint. Took: rule #8 — labels at 10–11pt with 0.18–0.22em tracking, content at 18–240pt. There is no middle weight. The `mid-label` style throughout the dashboard is direct MUJI/Hara DNA.

11. **Ezra Stoller architectural photography monographs**
    Photographic discipline matched by typographic discipline; structural alignment; everything sits where the architecture says it should sit. Took: the underlying visible grid, and the discipline that every right-aligned numerical column shares a single tabular axis — the alignment is structural, not stylistic.

## Notes

- I did not look at Aave / Drift / Jupiter / Phantom (per anti-references list) and actively designed *against* their conventions: no purple, no glassmorphism, no gradients, no rounded corners, no card chrome, no neon, no glyph.
- Inter (rsms.me/inter) is the typeface of record. It is the closest open variable to Helvetica Now Display 95 / Akzidenz-Grotesk Black at extreme weight; tabular numerals via `font-feature-settings: "tnum"` are non-negotiable for the financial data.
- Browser-based reference screenshots were skipped (Playwright was occupied at session start); references above are working URLs the orchestrator can follow.
