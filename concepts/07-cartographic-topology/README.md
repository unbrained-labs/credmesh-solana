# Concept 07 — Cartographic Topology

## Thesis

Credit has terrain. A liquidity pool is a watershed; utilization is its elevation profile; an agent's reputation is its altitude above sea level; a default is a crevasse, marked with the inward-facing hachure ticks cartographers use to denote a depression. The product is not a "DeFi dashboard" — it is a USGS 7.5' quadrangle sheet, edition 2026, of a country that does not appear on any other map.

The protocol is for autonomous agents, but the brand is for the human reading the chart before they let the agent walk into the wilderness. CredMesh is the survey: it tells you where the ridges are, where the wash-outs happened, where the path is well-trodden and where it has never been mapped at all. Pool fees scale with utilization the way air thins with altitude — quoted to you on the legend, not negotiated. Reputation is a benchmark elevation: stamped, dated, recoverable. Trust is geodetic.

This world borrows from a single visual lineage: the engraved hand of the U.S. Geological Survey and the European cartographic tradition (Imhof's Swiss Manner, Robinson's *Elements*, Tufte's *Envisioning Information*). It rejects the screen entirely. It sits on cream paper. The numbers are tabular lining figures because numbers on a map have always been lining figures. The blue is the cobalt of hydrography, never the blue of a button.

A tagline, set on the cartouche: **CredMesh — A survey of the credit terrain.**

## Design rules

1. **Two ink colors only, plus paper.** Sepia/brown (`#7A4E2D`) for line-work and contours. Cobalt (`#1B3A6F`) for hydrography and the single live data series. Paper (`#EFE3CB`) is always the background — never white, never black. Crevasse-red (`#6B2520`) is reserved exclusively for defaults and negative values; using it for anything else dilutes the warning.
2. **Never use a circle as a primary shape.** Circles are reserved for the benchmark disc (the logomark) and for control-point markers. Data is ALWAYS encoded as contour lines, hachures, or labelled tabular columns — never as donuts, never as pies, never as bubble charts.
3. **All labels are ALL CAPS, letter-spaced (~0.08em), Barlow Condensed Bold or equivalent.** This is the USGS label tradition. Mixed-case is acceptable only inside long-form prose blocks and inside the cartouche tagline.
4. **All numerals are JetBrains Mono, tabular lining figures.** A number on a map is a survey reading. It must align in columns and never reflow.
5. **Every screen has marginalia.** Title block top, scale bar, north-arrow rosette, declination diagram, contour interval note, datum note, edition date. The data sits inside the marginalia, not on top of it. If a screen has no margin, it is not a chart.
6. **Hairline rules, never boxes.** Use 0.5–1px sepia hairlines to divide. Never solid filled containers. Never drop shadows. Never rounded corners — corners are 90° because survey sheets fold flat.
7. **Defaults render as crevasses, not as red dots.** A defaulted advance gets the inward-pointing hachure tick treatment, three to seven ticks, labelled with the date the depression was surveyed. This rule is load-bearing: it makes risk visible the way a map makes a cliff visible — by drawing what is actually there.
8. **No animation except on update.** When a value changes, it gets a hand-stamped overprint: the old value is struck through with a sepia hairline, the new value appears beneath it with a "REVISED 2026.05.03" tag, exactly as a quad sheet annotates a corrected elevation. Nothing fades, nothing slides, nothing pulses.
9. **Reputation is elevation; never call it a score.** Throughout the UI: `ELEV. 1,847 m`, never `score: 1,847`. Coordinates of an agent are written as `40°47′N 73°58′W` — derived deterministically from their pubkey hash. Treat the agent like a control point that has been recovered in the field.
10. **The logomark is a benchmark disc. Never use a CredMesh "logo" without the disc engraving.** No isolated wordmark, no flat C-in-a-circle. The mark is meant to feel like it was struck with a chasing tool into bronze, not exported from a Figma plugin.

## What credit feels like in this world

You are about to lend liquidity. You unfold the sheet on a flat surface. You read the title block: edition date, datum, magnetic declination from true north. You find the watershed of Pool 001. You trace a contour: 67% utilization, the live cobalt isarithm. You note the ridge — that is the waterfall priority. You see, near the southwest corner, three crevasses surveyed last quarter. You read the bearing of the recent-advances register. You sign the chart in pencil and put it back in the drawer. The terrain is what it is. You decided whether to walk it.
