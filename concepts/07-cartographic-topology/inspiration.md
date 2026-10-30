# Inspiration · Concept 07 Cartographic Topology

References I actually drew on. Each note records what was lifted, not just admired.

## Cartographic primary sources

1. **USGS 7.5′ 1:24,000-scale historical quadrangle (Mt. McHenry's Peak, CO 1957/1992 — USGS sample)**
   https://www.usgs.gov/media/images/example-map-75-minute-124000-scale-historical-usgs-topographic-map-series
   The whole grammar is here: title block, neatline, marginalia strip, scale bar with alternating black/white half-cells, declination diagram, contour intervals labeled with hand-set lining figures. The dashboard's title block, marginalia, and corner-tick frame come directly from this layout.

2. **USGS Cobalt, ID 1:24,000 quad (1989/2011/2013)**
   https://www.yellowmaps.com/usgs/quad/45114a2.htm
   Naming the pool view as a "quadrangle" with named adjacent sheets ("Blackbird Creek NW", "Leesburg NE", etc.) is from this. The "Sheet 001 of XLVII" line in the title block borrows the convention.

3. **USGS Topographic Map Symbols (legend chart)**
   https://www.usgs.gov/programs/national-geospatial-program/topographic-maps
   The Legend column in the cartouche reuses these conventions: brown contour lines, blue hydrography (here: USDC flow), the upward-pointing triangle for triangulation/benchmark, hachure ticks for depressions/cliffs.

4. **NGS Survey Mark Recovery — "Mark Descriptions Help"**
   https://www.ngs.noaa.gov/surveys/mark-recovery/mark-descriptions-help.shtml
   Mark types: brass disc, triangulation station, reference disk, crosshair stamping, year-stamped editions. The benchmark logomark reads as a brass triangulation disc precisely because it follows this taxonomy: outer rim, inner field, inscribed triangle, central crosshair, year stamp.

5. **USGS Library Guides — Land Survey Benchmarks**
   https://libraryguides.usgs.gov/benchmarks/types
   Distinction between *triangulation station* (horizontal control) and *bench mark* (vertical control). The brand uses both: the agents are triangulation stations (their reputation is a position), the pool is a bench mark (TVL is an elevation). The vocabulary in the cartouche ("Horizontal Datum", "Vertical Datum") follows.

6. **Volcano Watch — "Bench marks: monuments of the past for future use"**
   https://www.usgs.gov/news/volcano-watch-bench-marks-monuments-past-future-use
   "A bench mark is a metallic disk that is cemented into bedrock... about 9 cm in diameter with a gentle convex surface defining a high point, usually marked by a cross within a triangle." This sentence is the logomark spec.

## Relief & elevation tradition

7. **Eduard Imhof — *Cartographic Relief Presentation* (1965, English ed. ESRI Press)**
   https://search.worldcat.org/title/cartographic-relief-presentation/oclc/824106975
   The "Swiss Manner" (oblique illumination upper-left, aerial-perspective hypsometric tints, contour lines + hachures + rock drawing): Rule 7 — defaults rendered as crevasses with hachure ticks — is direct Imhof. The contour stack on the dashboard layers tones-of-sepia from outer to inner with one cobalt live line, mirroring his rule: "Only simplicity provides a lasting impression."

8. **Eduard Imhof biographical & methodological essay — ICA**
   https://icaci.org/eduard-imhof-1895-1986/
   "In normal vision nearby landscape colours are brighter than those further away" — informs the choice to keep low-utilization contours as faded sepia and only the live ridge as saturated cobalt. Active = near, dormant = aerial-perspective haze.

9. **Cartographic Perspectives review of Imhof (Youngblood)**
   https://cartographicperspectives.org/index.php/journal/article/download/cp65-youngblood/pdf/976
   Chapters 8 (contour lines) and 11 (rock drawing / hachures) — the technical justification for using hachures (as opposed to pictograms) to mark the default crevasse. Ticks must point inward toward the lowest elevation; this is now Rule 7 of the design system.

## Hydrography & color

10. **NOAA Chart 12300 — Approaches to New York, Nantucket Shoals to Five Fathom Bank**
    https://www.charts.noaa.gov/OnLineViewer/12300.shtml
    Nautical-chart cobalt for water, sepia for land, black for type — and the discipline of marginalia (compass rose, depth datum, scale, magnetic declination, latitude/longitude tick labels in the corners). The four-corner coordinate labels and the latitude/longitude graticule on the contour map are lifted from this chart's neatline.

11. **NOAA Chart No. 1 — Nautical Chart Symbols & Abbreviations (PDF)**
    https://repository.library.noaa.gov/view/noaa/49552/noaa_49552_DS1.pdf
    The convention of abbreviating field names ("Hbr", "BM", "Δ" for triangulation) — the dashboard uses "BM Δ TVL ELEV." in the same compressed cartographic shorthand.

## Information design

12. **Edward Tufte — *Envisioning Information* (Graphics Press, 1990)**
    https://www.edwardtufte.com/tufte/books_ei
    Small multiples, high data-ink ratio, layering by line weight rather than color. The contour stack is one Tufte small-multiple compressed onto a single sheet; weight-based hierarchy (0.4 → 2.2px stroke for live) is the data-ink rule.

13. **Robinson, Morrison, Muehrcke et al. — *Elements of Cartography* (5th ed., Wiley)**
    https://www.wiley.com/en-us/Elements+of+Cartography%2C+5th+Edition-p-9780471509103
    Type hierarchy on maps: condensed sans (caps, letter-spaced) for cultural features, italic serifs for hydrography, body sans for prose. The brand restricts itself to the first — Barlow Condensed for everything labelled, JetBrains Mono for everything measured.

## Type & disc reference

14. **Trade Gothic Bold (the original Linotype, 1948 — Jackson Burke)**
    Used historically on USGS labels alongside Whitney/Univers. Barlow Condensed is the modern open-source proxy with similar X-height, condensed proportion, and engineering-bold weight. Picked because it's Google-Fonts–served and renders identically on every machine.

15. **JetBrains Mono — tabular lining figures**
    https://www.jetbrains.com/lp/mono/
    Picked because all numerals are equal-width with `font-feature-settings: "tnum" 1`. Survey readings must align in columns — see Rule 4.
