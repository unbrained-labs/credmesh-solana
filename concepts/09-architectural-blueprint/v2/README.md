# 09 — Architectural Blueprint · v2

## Thesis (refined)

Credit is a built thing. v1 said so by drawing the protocol on blueprint
emulsion paper — cyan-navy sheets, cream linework. The risk it ran was
**fatigue at full app scale**: a working surface that's 40% blue, 60% blue,
80% blue gets visually heavy after thirty seconds of reading addresses on it.

v2 keeps the metaphor and **inverts the load path**. The canvas is now
drafting paper — warm cream, slightly aged — and blueprint cyan becomes
the **drawing layer**: sheet titles, dimension lines, callouts, schedule
rules, registration crosses. The data sits ON the cream. The blueprint
is the chrome, not the background.

The cyan still owns one zone — the **TVL hero panel** — where the page
folds back to a traditional blueprint inset. The inset is what proves the
canvas is the brand: the moment the data demands hero treatment, we cut
to a blueprint sheet. Everywhere else, the data reads like a working
drawing on engineering paper.

> EVERY FLOW HAS A LOAD PATH.
>
> *(refined: every value has a witness rule.)*

---

## What's in this folder

| File | Role |
|---|---|
| `tokens.css` | Single source of truth — color, type, spacing, hatching, base atoms |
| `elements.html` | Primitive showcase · designer handoff sheet · 16 specimen rows |
| `dashboard-frame.html` | LP Dashboard (UX_SPEC §2.1) — refined, info-denser than v1 |
| `agent-observatory.html` | NEW — human-watching-machine view (UX_SPEC §2.2) |
| `governance.html` | Governance / drawing revisions (UX_SPEC §2.3) |
| `README.md` | This document |

All HTML files import a single stylesheet (`tokens.css`) — no inline
duplicates.

---

## 1. Color tokens

The page is paper, not screen. Cream is warm and slightly aged
(`#ECE3CD`), never pure white. Ink is blueprint navy (`#1B3A6F`), never
pure black. Hatched fills carry weight where solids would on a normal
page; we never use solids.

```css
--paper:        #ECE3CD;   /* primary canvas */
--paper-warm:   #E2D7BC;   /* secondary panel / id-strip background */
--paper-deep:   #D7C9A6;   /* hatched / inset banding */
--paper-edge:   #C9B98F;   /* deckle / margin */

--ink:          #1B3A6F;   /* blueprint navy — primary ink */
--ink-deep:     #0F2347;   /* heavy linework, table cells */
--ink-soft:     rgba(27,58,111,0.62);   /* secondary labels */
--ink-faint:    rgba(27,58,111,0.35);   /* axis labels, micro rules */
--ink-ghost:    rgba(27,58,111,0.16);   /* table inner rules */

--blueprint:        #1B3A6F;   /* the inset — TVL hero, key diagrams */
--blueprint-ink:    #F2EDE0;   /* cream linework on inset */
--blueprint-soft:   rgba(242,237,224,0.62);

--stamp:        #A8342B;   /* destructive only, void/frozen, danger gate */
--amber:        #B26A1F;   /* pending, timelock, oracle stale */
```

**Rules of use** — the discipline matters more than the swatches:

1. The page background is always `--paper`. Never use cyan as the canvas.
2. The blueprint inset (`--blueprint`) appears in **at most one zone per
   sheet** — typically the hero number. It's a hand-cut inset, not a
   panel style.
3. `--stamp` is reserved for destructive-action chrome and "VOIDED /
   REJECTED / FROZEN" status callouts. Never use stamp red as a hover
   color or for "important" things that aren't dangerous.
4. `--amber` is reserved for time-bound pending states (timelock, oracle
   staleness, pending tx confirmation). Not for "selected".
5. **No solid fills for state.** Selected / active / warning zones are
   hatched (`--hatch-mid`, `--hatch-stamp`, `--hatch-amber`).

---

## 2. Type ladder

Three faces only. Every face has one job:

| Face | Used for | Never used for |
|---|---|---|
| **Barlow Condensed** (caps) | labels, sheet titles, button copy | numbers, addresses |
| **JetBrains Mono** | every number, address, slot, sig, error name | running prose |
| **Architects Daughter** | margin notes, hand-drawn annotations | data, headlines |

Named scale (px), with role mapping:

```
--t-micro    8.5    registration marks, witness labels
--t-mini     9.5    schedule column heads, dim labels, status pill
--t-tiny    10.5    table cells, sub-stats, titleblock keys
--t-fine    11.5    schedule body data, primary table numbers
--t-base    13      paragraph, marginalia (Hand)
--t-md      16      stat-card label, action-card heading
--t-lg      22      sheet title (top of every page)
--t-xl      28      stat-card hero (Share Price = 1.0234)
--t-2xl     42      sub-hero (Outstanding = $2,845,624)
--t-3xl     72      TVL hero on dashboard inset
```

**Numeric treatment:** every number is JetBrains Mono with `font-feature-
settings: 'tnum' on, 'lnum' on, 'zero' on` (tabular figures, lining
figures, slashed zero). Numbers right-align in schedules. USDC quantities
display 4 dp by default.

**Numbers carry units.** `1.0234` is a sin; `1.0234 USDC/SH` is the rule.
Units render at `0.62em` of the number, letter-spaced `0.32em`, in
`--ink-soft`.

---

## 3. Spacing scale

4-px module. Layouts snap to it.

```
--s-1   4px     row gap, hairline pad
--s-2   8px     table cell pad, primitive inner gap
--s-3  12px     primitive inner pad
--s-4  16px     stat-card body pad
--s-5  24px     panel separation
--s-6  32px     section break (sheet header → body)
--s-7  48px     sheet margin (1440 sheet → 36 + content + 36)
--s-8  64px     hero offset
--s-9  96px     reserved (multi-page templates)
```

Sheet outer margin is 36px; that's `--s-6 + s-1` and ties to the
registration-cross frame at 18px / 22px.

---

## 4. Primitives this concept owns uniquely

These are the things that make the page read as a drawing rather than a
generic dashboard. Every one is rendered in `elements.html` with a label.

| # | Primitive | Why it's load-bearing |
|---|---|---|
| 1 | **Sheet header** with PROJECT · SHEET N OF 12 · CLUSTER + TITLE + DRAWN/CHECKED/REV | Every page is a sheet. The header is its mast. |
| 2 | **Title block** (bottom-right, fixed cells) | Sacred — never resized, never decorated. |
| 3 | **Registration crosses** at every framed corner (`+`) | The page's grammar — they're not optional. |
| 4 | **Section markers** (Ⓐ Ⓑ Ⓒ … and Greek α β γ δ for the observatory) | Indexes content to its callout / caption. |
| 5 | **Dimension line** (witness ticks + hairline rule + value floating above) | Every quantity earns a witness. Never a naked number. |
| 6 | **Hatched fills** (45° hairline, three densities) | Status / selected / warning, never a solid. |
| 7 | **Status stamp** (caps, hairline border, 4 intensities: ink / amber / stamp / invert) | The status pill in this world. |
| 8 | **Numbered callout** with leader line (`¹` + body) | Pointing at a thing on the sheet. |
| 9 | **Schedule** (table) — caps headers, mono body, hairline rules, no zebra | Tables are schedules. |
| 10 | **Identity strip** (α/Σ/M markers + role + pubkey + stamp) | Pubkey is load-bearing data, never metadata. |
| 11 | **Lifecycle bar** (Sent · Pending · Finalized — three stages, hatched / warm / blank) | Tx lifecycle never collapses to a check. |
| 12 | **Destructive gate** (stamp-red border 1.5px + hatch overlay + CHECKED ☐ APPROVED ☐ rubric) | The strongest chrome on the sheet. |
| 13 | **North arrow** labeled `N: CONSENSUS` | The page has an orientation. Capital flows up. |
| 14 | **Margin note** in Architects Daughter | The engineer working something out by hand. |
| 15 | **Blueprint inset** (cream-on-cyan zone) — TVL hero only | One inset per sheet. The brand cuts back when it earns it. |
| 16 | **Drawing revision card** (proposal as `R-093`) | Governance proposals are sheet revisions. |

---

## 5. Information density · how v2 is sharper than v1

The brief asked v2 to be **demonstrably better** than v1 at info density
and at first-classing hex addresses. Concrete deltas:

### Dashboard

| | v1 | v2 |
|---|---|---|
| TVL hero treatment | 112pt cream-on-blue, full canvas | 96pt cream-on-blue inset over cream canvas; the cyan returns *for the hero* and recedes everywhere else |
| Secondary metrics | 4 metrics in a single horizontal strip, no stamps | 4 metrics in a 2×2 grid with status stamps (Healthy / Below kink / Pool open / Within tol.) — same area, more data |
| Time-series | Utilization curve, 24h, single trace | Share-price curve, 30d (per UX_SPEC), with hover-witness vertical and slot+timestamp+price+delta tooltip-row |
| Yield attribution | Not present in v1 | New panel (UX_SPEC §2.1) — 4-row breakdown by source with hatch bars and 24h totals |
| Action panel | Present but no lifecycle | New 3-stage lifecycle (Sent · Pending · Finalized) with slot at each stage, fee-payer pubkey, sig truncation |
| Settlements | 6 rows, status as single word | 8 rows, +SOURCE column (STRIPE/WIRE/ACH), +tx sig column, status as 4-state stamp (Settled / Pending / Voided + dim) |

### Agent observatory (NEW — does not exist in v1)

A page where the **majority of glyphs are hex**:

- **Identity strip** — three pubkeys side-by-side with full + truncated, role, audit-chain status
- **Vitals card** — 4 cells (debt · digest · advances · last-onchain) each dimensioned with units and slot
- **Reputation timeline** — stepped (not interpolated, per UX_SPEC §2.2), 8 transitions across 30d, hatched under the line, square markers at each digest transition, hover-witness shows digest + delta
- **Health panel** — 4 cells (oracle staleness · idle USDC · key rotation due · circuit breakers); the warning ones get amber hatch-overlays
- **Recent advances schedule** — 10 rows, slot opened + slot settled both shown; "open" as a literal value, never an empty cell
- **Receivable inflow log** — 10 rows, payer pubkey AND tx sig both truncated and visible (the page is designed for grep-by-eye)
- **Action footer** — three actions in a row: manual repay (default) · withdraw idle USDC (primary CTA) · emergency exit (the destructive gate, with stamp-red border 1.5px, 4-item check rubric, hatch overlay)

### Governance

- **Proposals as drawing revisions** — each card has its own `R-NNN` number rendered at 28pt, large enough to scan from across a screen
- **Three bands** — Active · In Timelock · Recently Executed — with `EXECUTED` and `REJECTED` stamps drawn at 28pt over the historical cards (rotated 8°, hatched)
- **Tally with quorum line** — three vote bars (FOR / AGAINST / ABSTAIN), each with hatch fill (mid / stamp / sparse) so it's legible without color-coding
- **Parameter schedule** — 9 rows, P1/P2/P3 tier stamps, "pinned at slot" column, "Δ since" linking to the proposal that last set the value
- **History diff** — 8 of 91 last revisions, rendered like git-diff (`<strikethrough> from</strikethrough> ⇢ <bold>to</bold>`), each with its slot

---

## 6. Design rules that survived v1 unchanged

These were correct in v1 and are preserved verbatim:

1. **Hairlines only.** 0.5px chrome, 0.75–1.0px primary linework, 1.5px
   stamp-red borders on destructive zones. Weight comes from layering.
2. **Three typefaces only.** Adding a fourth weakens the discipline.
3. **No solid fills.** Hatching always.
4. **No icons.** Components are drawn as the *thing* — vessel for pool,
   beam for reputation, fastener for advance. If you can't draw it as
   an architectural element, it doesn't belong.
5. **Title block lives bottom-right, fixed dimensions.** Never resized.
6. **Section markers and registration crosses are not optional.** They
   are the page's grammar.
7. **Tables are schedules.** Caps headers, mono body, hairline rules
   between rows, no zebra, no row hover.
8. **The North arrow points up, labeled `N: CONSENSUS`.** Always.
9. **No animation, no glow, no gradient.** The page is paper.

---

## 7. Anti-rules (every concept · agent-vs-human discipline)

Carrying forward from v1 + UX_SPEC §10:

- No purple. No glassmorphism. No 3D depth. No emoji. No fake AI mascots.
- No "Connect Wallet" as a feature button — wallet is sheet chrome.
- No naked numbers. Every quantity carries a unit.
- No collapsed lifecycle. Sent → Pending → Finalized, three stages, slot
  at each.
- No bare "are you sure?" modal. Destructive actions go through the
  CHECKED ☐ APPROVED ☐ rubric in stamp-red chrome.
- Pubkey truncation is `4 + … + 4` always. Never one-end-only.
- Solana errors render as `CredmeshError::InvalidFeeCurve` *and* `0x1771`,
  never just the number.

---

## 8. Open questions / nice-to-haves not in this v2

These would be next-pass work, not blockers:

- **Mobile observatory** — the page is currently 1440-fixed. A column-
  collapse strategy for sub-1024 widths would mean choosing which
  schedules fold and which stay first-class. The dimension-line atom
  doesn't survive a 320px column without redesign.
- **Print stylesheet** — the metaphor begs for a literal print path.
  `@media print` could ship a real PDF blueprint with this layout
  unchanged; mostly a matter of disabling shadows.
- **Live cursor witness** — the chart hover-witness is rendered static
  here; in build, it should track the cursor and draw the witness rule
  + slot label at the actual hovered point.
- **Public Stats** and **Marketing landing** — UX_SPEC §2.4/2.5 — left
  for a later pass, by intention.
