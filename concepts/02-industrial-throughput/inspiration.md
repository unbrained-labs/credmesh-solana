# inspiration · 02-industrial-throughput

## Visual references in `inspiration/`

The Playwright browser was occupied by another worker during this session, so
I could not capture live screenshots of the references below. In place of
external screenshots I rendered two **mood specimens** locally that show
exactly which palette + motifs + type ladder I lifted, so a reviewer can see
what got carried across:

- `mood-01-palette-and-motifs.svg` — full palette (bone / iron / brass / fume /
  alarm), the mechanical-counter face (fume on iron, the "gas-pump readout"),
  and the four P&ID motifs the world is built from (relief valve, pressure
  gauge, flanged-pipe + instrument tag bubble, paper-fed strip chart).
- `mood-02-typography-and-grid.svg` — IBM Plex Mono type ladder at the brand
  sizes, the engineering-vellum 8px crosshatch under the 64px bold grid, a
  full-size mechanical TVL counter, and a sample of the "maintenance-log"
  voice that replaces marketing copy.

## References used (cited specifically)

1. **Honeywell Instrumentation Handbook, 1970** — Computer History Museum
   collection 102685255.
   <https://www.computerhistory.org/collections/catalog/102685255>
   *Took:* the cabinet-typography ratio (instrument tag in mono caps, value
   below in larger weight, tiny unit suffix), and the idea that every readout
   sits inside a bordered cell with a tag code (`PI-117`, `LIC-001`).

2. **Apollo 11 Final Flight Plan, 1969-07-01** — NASA HQ, archived by
   Honeysuckle Creek and the National Archives.
   <https://www.nasa.gov/wp-content/uploads/static/apollo50th/pdf/a11final-fltpln.pdf>
   <https://www.archives.gov/exhibits/featured-documents/apollo-11-flight-plan/flight-plan.html>
   *Took:* the title-block grid (sheet number, revision letter, operator
   initials in a meta-cell on the right of the header), the absolute
   timestamp every row, and the dry, hour-stamped "schedule of tasks" tone
   that the dashboard's footer log adopts.

3. **NASA Contractor Report 177605 — "On the Typography of Flight-Deck
   Documentation," Asaf Degani, 1992.**
   <https://ntrs.nasa.gov/api/citations/19930010781/downloads/19930010781.pdf>
   *Took:* permission to use ALL CAPS for status lines (the report defends
   it for safety-critical reading), tabular figures everywhere, and tight
   line-length on log entries.

4. **GOST 21.404-85 — Soviet "Designations for instruments and means of
   automation in process automation schemes."**
   <https://ani-studio.narod.ru/BOX/Flash/Study/Automation/HTML-Themes/DOCs/GOST21.404-85.htm>
   *Took:* the convention of letter-prefix instrument tags
   (P=pressure, F=flow, L=level, K=count, X=ratio, R=record, I=indicate,
   C=control). The dashboard codes — `PI-117`, `LIC-001`, `FT-204`,
   `XI-002`, `KI-301`, `RIC-301` — are all drawn from this scheme.

5. **ISA S5.1 — Instrumentation Symbols and Identification.**
   *Took:* the **two-opposed-triangles** glyph used as the relief-valve
   logomark, and the round "instrument balloon" tag (circle + horizontal rule
   + two-letter function code over a number).

6. **Edrawsoft P&ID Symbols Legend (PDF).**
   <https://www.edrawsoft.com/pid/images/pid-legend.pdf>
   *Took:* line-weight conventions — 0.5px schematic, 1px vessel wall, 2px
   primary flow — encoded as Rule 5 in the README.

7. **Vignelli + Knoll graphic program, 1967–71.** Knoll au Louvre catalogue.
   <http://www.archiviograficaitaliana.com/project/237/knollaulouvre>
   <https://dedece.com/knoll-by-vignelli/>
   *Took:* one type family carrying every hierarchy through scale and
   weight only; uppercase + tight letter-spacing for institutional voice;
   black/white/single-warm-accent (CredMesh's accent is brass, not Vignelli's
   red, but the discipline is the same).

8. **Obninsk Atomic Power Station — central control panel photograph,
   Library of Congress LC-USZ62-77531.**
   <https://hdl.loc.gov/loc.pnp/cph.3b24664>
   *Took:* the "wall of identical bezels" composition that informed the
   four-up readout row, and the brass-on-cream tonality.

9. **Veeder-Root mechanical totalizer (vintage gas-pump number wheels).**
   <https://www.gaspumps.us/product/veeder-root-front-totalizer-good-wheels/>
   <https://www.signs101.com/threads/old-gas-pump-numbers.159169/>
   *Took:* the digit-cell counter aesthetic — fume-yellow numerals on a
   near-black ground with a slim vertical rule between every wheel and a
   small inset shadow at the top of each digit window.

10. **ElektroMera / VNIITE 1973–79 — Margareta Tillberg, Baltic Worlds.**
    <https://balticworlds.com/design-of-electronicelectrical-systems-in-the-soviet-union-from-khrushchev%e2%80%99s-thaw-to-gorbachev%e2%80%99s-perestroika/>
    *Took:* the meta-rule that an industrial brand exists to standardize a
    fleet of dissimilar instruments — that's CredMesh too. Many liquidity
    sources, many agents, one chassis.

11. **Edward Tufte — *Beautiful Evidence* (2006), strip-chart and sparkline
    chapters.**
    *Took:* annotation-as-flag-on-a-leader-line on the throughput chart
    ("PEAK · 51 ADV/h"), the visible mean reference line (μ = 24.7), and the
    refusal to use a smooth area chart where a paper-fed strip chart will do.

12. **Mike Abbink + Bold Monday — IBM Plex Mono, 2017.**
    <https://www.ibm.com/plex/>
    *Took:* the only typeface in the system. Its slightly humanist mono feel
    keeps the dashboard from reading as harsh OCR-A.
