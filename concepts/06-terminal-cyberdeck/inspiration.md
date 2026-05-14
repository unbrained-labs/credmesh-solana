# Inspiration — 06 / terminal-cyberdeck

References I actually drew from. Everything else was filtered out.

## Lineage references

1. **Bloomberg Terminal — origin story (1981).**
   <https://www.bloomberg.com/company/stories/innovating-a-modern-icon-how-bloomberg-keeps-the-terminal-cutting-edge/>
   The hose-cabled keyboard, the closet of custom hardware, the monochrome
   yield-curve plotted in line strokes. Took: the *belief that information
   density beats decoration*. The Terminal has resisted thirty years of
   redesign because the data was already the design. CRED.MESH inherits
   that posture.

2. **Bloomberg amber-vs-green folklore (HN thread).**
   <https://news.ycombinator.com/item?id=19155772>
   Bloomberg picked amber to be distinguishable from every other terminal
   in 1981. Took: the **amber primary**, both as visual mark of "this is
   the Bloomberg of agent credit" and because amber is famously easier on
   the eyes at full-day length than phosphor green for chart-dense screens.

3. **btop — README screenshot.**
   <https://raw.githubusercontent.com/aristocratos/btop/main/Img/normal.png>
   See `inspiration/screenshot-04-btop-tui.png`. Took: the **panel grammar**
   (single-line ┌─┐ corners, panel labels in superscript, mini sparkline
   axes labeled `²mem` `³net` etc), the **braille-and-block charts**,
   right-aligned tabular numerics. The dashboard's `UTIL.30D` and `30D
   PROFILE` rows are direct btop-grammar borrowings, recoded for finance.

4. **The Cyberdeck Cafe — "M3TAL" build.**
   <https://cyberdeck.cafe/mix/m3tal>
   See `inspiration/screenshot-03-cyberdeck-m3tal.png`. Hand-soldered
   keyboard, exposed extrusion, "no glue in this build at all." Took: the
   **logo's PCB-as-logomark concept** — through-hole pads, copper traces,
   silkscreen-white reference text, fiducial corner marks, an SMD chip
   nestled between the C and the M for scale.

5. **Sci-Fi Interfaces on *Hackers* (1995).**
   <https://scifiinterfaces.com/2023/12/11/hackers/>
   See `inspiration/screenshot-02-hackers-ui.png`. "There is no hidden
   surface removal, no lighting, no shadows. Just straight lines and plain
   text." Took: the **rule against gradients, glass, and depth shadows**.
   The CRT glow on text is the only luminous element on the page — the
   chrome stays flat, axis-aligned, plain.

6. **Joe Clark on the graphic design of *Hackers*.**
   <https://joeclark.org/writing/film/hackers-design.html>
   Neville Brody's screen designs that "vary in implausibility from
   modest to hopeless" — but stayed legible because they refused to
   imitate any 1995 OS. Took: the principle that **a brand built around
   software should not look like the software it runs against**. CRED.MESH
   doesn't try to look like Phantom or Jupiter. It looks like the room a
   sysadmin sits in.

## Component-level references

7. **Departure Mono — typeface specimen.**
   <https://departuremono.com/>
   Box-drawing characters baked into the typeface, "8-bit video game or
   ASCII art" specimen tone. Took: the *mission-report* layout idea —
   header label, dashes, telegraphic line ("EARTH DATE: NOV 20, 2057")
   pattern. Couldn't ship Departure Mono itself (no Google Fonts CDN), so
   I went with **JetBrains Mono** for body and **VT323** for hero readouts
   to get the same CRT-bitmap lineage.

8. **Iosevka — terminal-grade specimen.**
   <https://typeof.net/Iosevka/specimen>
   Reference for *which monospace conventions to keep*: tabular numerals
   on by default, narrow column packing, terminal-mode bracket pairs.
   Influenced the `font-feature-settings: "tnum"` + tight letter-spacing
   on the dashboard.

9. **SymbolFYI — Box Drawing Characters guide.**
   <https://symbolfyi.com/guides/box-drawing-characters/>
   Heavy vs light box-drawing distinction (`┃` U+2503 vs `│` U+2502),
   the rule that "monospace is non-negotiable." Took: the **two-weight
   hierarchy** — heavy `┏━━┓` lines for the brand header, light `┌──┐`
   for sub-panels. It's the box-drawing equivalent of `<h1>` vs `<h2>`.

10. **UnicodeFYI — Block Elements reference.**
    <https://unicodefyi.com/guide/box-drawing-block-elements/>
    The eight-step block ramp `▁▂▃▄▅▆▇█` and the braille-pattern
    sub-cell density trick. Used the block ramp directly for the 30D TVL
    sparkline; used solid `█` for the utilization bar so the green fill
    reads as "voltage" rather than "progress."

## Anti-references (what I actively refused)

- **No purple gradients, no glassmorphism, no neon-on-dark blue.** The
  brand brief listed Aave/Drift/Jupiter/Phantom as anti-references and
  this concept already disagrees with them on first principles — but
  worth noting that I checked my own draft against this list at every
  step.
- **No "futuristic" sans-serif headers.** Eurostile, Orbitron, Audiowide,
  any of the "cyber" Google Fonts — banned. The terminal voice is
  monospace or it's nothing.
- **No literal pictogram logo.** No node-and-edge mesh icon. No coin. The
  PCB-as-logomark *is* the brand metaphor: a thing that was actually
  soldered, that has copper traces and drilled holes and silkscreen
  reference text. Credit as a *circuit*, not as a token.
