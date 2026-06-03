# Inspiration — 10 Swiss Institutional

References actually used. Each line: URL — what was lifted.

1. **ECB Economic Bulletin, Issue 7 / 2025 (PDF)** — https://www.ecb.europa.eu/pub/pdf/ecbu/eb202507.en.pdf
   The whole layout vocabulary: numbered sections (§1, §2…), the chart-caption convention (`Chart 1.A — …, Source:`), the right-aligned "Issue / 2025" stamp on every page. The masthead structure of the dashboard is essentially this PDF reflowed for HTML.

2. **ECB Economic Bulletin landing page** — https://www.ecb.europa.eu/press/economic-bulletin/html/index.en.html
   Confirms how a central bank treats a recurring publication as a *series* — Issue, Year, page numbering. Reused as `Statistical Bulletin · No. 1 / 2026`.

3. **Bundesbank `bbkplot` corporate-design paper** — https://www.bundesbank.de/resource/blob/831408/20861a1d419d93a1b2eba25ee829eae1/mL/2020-03-bbkplot-data.pdf
   Confirmed the rules I'd half-remembered: 9–12pt continuous text; supplementary text at 70 % of title size; specified colour palette; titles in Frutiger Next Medium / Light. I substituted Helvetica per the seed but kept the relative type-scale ratios (44pt → 18pt → 12pt → 10pt → 9pt). Also took the two-rule (`heavyrule`) divider convention from page-break design in their plots.

4. **Bundesbank Monthly Reports index** — https://www.bundesbank.de/en/publications/reports/monthly-reports/current-monthly-reports--764440
   "Monthly Report" cadence — used to justify a *bulletin* framing rather than a *dashboard* framing. The snapshot timestamp footer with slot/block number is the on-chain analogue of "compiled at close of business 03.05.2026."

5. **BIS Quarterly Review, December 2025 (PDF)** — https://www.bis.org/publ/qtrpdf/r_qt2512.pdf
   Took the *Notations used in this Review* convention (e = estimated, lhs/rhs, "Differences in totals are due to rounding") — informs the small-print discipline in the §3 footnote ("Counts of 47 active advances at snapshot"). Also: the matter-of-fact two-line section headers `§ N — title / subtitle`.

6. **BIS Quarterly Review March 2026 PDF** — https://www.bis.org/publ/qtrpdf/r_qt2603.pdf
   Endnote-graph cross-referencing pattern (`Graph 1.A: EA = STOXX Europe 600; …`) — informs my chart caption naming `Chart 2.A` and the §-prefix throughout.

7. **Müller-Brockmann, *Grid Systems in Graphic Design*** — https://www.typotheque.com/books/grid-systems-in-graphic-design
   Cited explicitly in the seed. Took: the discipline of column-based layout, the use of the rule (not the box) as the primary divider, and the principle that body type and footnote type sit in the same visual family at different sizes — never different fonts. The 12-column grid in the dashboard is straight from this book.

8. **Vignelli/Noorda, *NYCTA Graphics Standards Manual* (1970)** — https://en.wikipedia.org/wiki/New_York_City_Subway_map (and the 2014 Standards Manual reissue)
   Confidence to use Helvetica only, all-caps mastheads, and a *single accent colour*. Vignelli's NYCTA proves that a public-utility brand can run on Helvetica + black + one ink colour without losing recognition — exactly what CredMesh wants when dressed as a registry.

9. **Massimo Vignelli's NYC Subway Diagram (1972)** — https://www.minniemuse.com/articles/musings/the-subway-map
   The "abstract simplicity" doctrine: 45° / 90° angles, every line bends at the same angle, no decorative elements. The dashboard's right-angles-only rule (no rounded corners anywhere) is the Vignelli rule transposed.

10. **Swiss Re *sigma explorer* — Forecasts page** — https://sigma.swissre.com/
    The *only* live financial site I found that lays out forecasts as plain numeric tables (Real GDP growth, CPI, central-bank rates) without sparkline candy. Borrowed the no-frills `World / United States / Euro area / China` table styling for the §1 stat block treatment.

11. **Karl Gerstner, *Designing Programmes*** — https://www.lars-mueller-publishers.com/designing-programmes
    Mentioned in the seed. Shaped the conviction that the design *is* the system, not a skin: every numeric cell follows the same rule (right-aligned, tabular figures, decimal-aligned), every section the same template. No bespoke layouts.

12. **Knoll Bauhaus Reissue catalogues** (referenced in seed)
    Confirmed: thin sans-serif annotations against generous whitespace, never a colour overlay, photography (or chart) sits inside a hairline frame, captions live in 9–10pt below.

13. **Helvetica documentary (Hustwit, 2007)** — https://www.hustwit.com/helvetica
    Re-grounded the rule "if you find yourself reaching for a second typeface, you've already lost." Wordmark, mastheads, body, footnotes — one family, four weights.

## Reference screenshots

The shared Playwright browser was locked by another concept worker for this run, so no `.png` references are saved in `inspiration/`. The PDFs above (especially `ecbu/eb202507.en.pdf` p. 1 — masthead, and `qtrpdf/r_qt2512.pdf` p. 5 — chart layout) are the canonical visual targets and can be opened directly in any PDF viewer to compare against `dashboard-frame.html`.
