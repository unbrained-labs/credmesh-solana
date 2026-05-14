# Inspiration — concept 05 / lab-instrument

References actually used. Not "inspirations" in the moodboard sense — specific
artifacts whose decisions I copied or adapted, with a one-line note on what
made it into the world.

---

1. **Tektronix 475A Oscilloscope Service Manual (1976)**
   https://w140.com/tekwiki/images/6/62/070-2162-00.pdf
   The control-panel labeling style — small caps, monospaced, with technical
   abbreviations ("BW/TRIG VIEW", "VERT MODE") sitting next to the knob they
   command. I lifted the convention of every plate having a P/N and REV mark,
   and the idea that a screen with a stepped trace is the brand's hero shape.

2. **Tektronix 221 Oscilloscope Service Manual (1973)**
   https://w140.com/tekwiki/images/f/fe/070-1573-01.pdf
   The "calibrate every 1000 hours" cadence and the calibration-certificate
   stamp idea. Drove the footer plate with `CAL ✓ VALID` and the
   "traceable to slot" line. Also the warm-paper/teal-bezel palette from
   scanned panel photos.

3. **Apollo 11 Final Flight Plan, NASA, July 1 1969**
   https://www.nasa.gov/wp-content/uploads/static/apollo50th/pdf/a11final-fltpln.pdf
   Five-column tabular layout (CMP / time / CDR-LMP / MCC-H), every cell
   labeled with an explicit role. Drove the "every column has a unit, no
   defaults" rule and the tabular density of the recent-advances plate.
   Page-numbering style `1-7`, `2-2` became my plate part-numbering
   `CM-201`, `CM-301`.

4. **National Archives — Apollo 11 Flight Plan exhibit (column key)**
   https://archives.gov/exhibits/featured-documents/apollo-11-flight-plan/flight-plan.html
   The colored column-key exhibit page. Confirmed for me that even when
   color is added, the underlying grid does the work — I kept teal as a
   pure flag color (header strip, fill markers) and used red ONLY for
   defaults / redline.

5. **HP Journal Index, 1973 (HP 3470 multimeter, HP 8640A signal generator era)**
   https://historycenter.agilent.com/pub-guides/hp-journal-index/hp1973
   Stylebook for the headline language: "A Greater Range of Capabilities for
   the Compact, Plug-on Digital Multimeter" — the deadpan, datasheet voice
   for the README's "Voice" section. Also drove the working title
   *Test Set 100-USDC*.

6. **Tufte — Visual Display of Quantitative Information, Ch. 2 & 5**
   https://lmscontent.embanet.com/USC/CMGT587/Tufte%20Ch2%20and%205.pdf
   "Dark grid lines are chartjunk … the grid should usually be muted."
   I used 0.5px warm-gray gridlines and only let the trace and the redline
   carry color. Also drove "annotate the line, not the corner" — see the
   PEAK 78.10% annotation directly on the curve.

7. **Tufte notebook — Table Graphics**
   https://edwardtufte.com/notebook/table-graphics
   The reputation cell uses a Tufte table-graphic: a sparkbar inline with
   the score number, scaled to full-scale (10,000), so the column is
   simultaneously a number and a small chart.

8. **Massimo Vignelli — Knoll International Poster (1967), Met Museum**
   https://www.metmuseum.org/art/collection/search/711046
   Two type families, ruthless grid, scale as the only display device.
   I set the rule: two typefaces, no third. Also the use of a dense
   horizontal type-block (the foot strip on the logo) as a graphic mass.

9. **Vignelli at Knoll — Knoll au Louvre poster, 1972 (Knoll editorial)**
   https://www.knoll.com/knollnewsdetail/iconic-knoll-graphic-installed-at-knoll-product-development
   "Discipline of the grid, visual power in the use of scale and color."
   Justified the decision to lean on numerical scale (54px display digit
   next to 9px monocaps unit) rather than decoration.

10. **Cooper Hewitt — Vignelli at Knoll, 2013**
    https://www.cooperhewitt.org/2013/04/18/a-colorful-identity/
    "Detests trends … causes waste and visual pollution." Direct license
    to refuse glassmorphism, gradients, and the rest of the anti-references.

11. **Juice Analytics — Better Know a Visualization: Small Multiples**
    https://www.juiceanalytics.com/writing/better-know-visualization-small-multiples
    Reminded me to keep "share price", "TVL", "utilization" as a row of
    same-shape plates. Same scale-frame, different measure. Row 1 of the
    dashboard is, structurally, three small multiples.

---

## Notes on what I deliberately did NOT take

- No Aave / Drift / Jupiter screen captures. Anti-reference per brief.
- No oscilloscope-trace gradient (real scope phosphor glows; we don't
  fake glow with a gradient — flat fill or nothing).
- No emoji, no rounded corners, no avatars. Per the design rules.
