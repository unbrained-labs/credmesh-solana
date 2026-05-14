---
concept: 05-lab-instrument
working title: CredMesh / Test Set 100-USDC
---

# Lab-instrument

## Thesis: credit as a measurement

CredMesh is a precision instrument — a **test set**, a calibrated piece of
bench equipment whose face you read and whose calibration sticker you
trust. An agent borrowing USDC against a signed receivable is, physically,
a current flowing through a known resistance. Our job is to display that
current to four decimals, label its unit, and make drift legible against a
printed scale.

The brand-world is a Hewlett-Packard test gear catalog (1973) crossed with
the Apollo 11 Flight Plan: every column labeled, nothing implicit.
Vignelli's Knoll grid sets the structure; Tufte is the conscience.

Credit, here, is a quantity — with a unit, against a standard, with a
margin of error. Reputation is a panel meter. Utilization is an
oscilloscope trace. Every figure carries a unit, because **numbers without
units are noise.** We court the human who picks the protocol: technical,
opinionated about typography, trusting an interface that respects them
enough to label the axes.

## Design rules

1. **Two typefaces.** IBM Plex Mono for numerals, IBM Plex Sans for prose. No third face. No italics except variable names.
2. **Every number carries its unit** in lower-opacity Mono, half a space away. `1,234,567.8201 USDC`, `67.42 %`, `412 bps`. Naked numbers are forbidden.
3. **Four decimals on share price, two on percentages, zero on slot counts.** Precision is design; under-precision is a lie.
4. **Five colors only.** Teal `#1F5C66`, warm gray `#8E8A82`, ink `#0C0E10`, paper `#F2EFEA`, alarm red `#C62828`. No gradients, ever.
5. **Hairlines 0.5px, rules 1px, bezels 2px, heavy bezels 4px.** Borders carry hierarchy.
6. **No circles as primary shapes** — except dial tics and corner fiducials. Cards are rectangles, 90° corners. Avatars do not exist.
7. **Every panel is a "plate"** with a part number top-right: `P/N CM-204 REV B`. Plates are named, not pages.
8. **Charts use a 5-tic axis with the unit on the axis line.** No legend boxes. Annotate the line, not the corner.
9. **Animation is mechanical, not eased.** Values count up in 60ms steps like a Nixie tube. Loading is a horizontal sweep, not a spinner.
10. **The page corners bear fiducials and a calibration date.** `CAL 2026-05-03 / TRACEABLE TO MAINNET SLOT 268,401,772`. Trust is a stamp.

## Sample tagline

> **CredMesh — Calibrated credit for autonomous agents.**
> Every advance, measured to four decimals. Every reputation, traceable to a slot.

## Voice

Datasheet, not marketing. *"Pool fee: 1 — 250 bps, utilization-indexed."*
The reader is a competent operator. We don't say "seamless"; we give them
the manual.
