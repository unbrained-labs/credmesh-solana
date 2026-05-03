# 10 — Swiss Institutional · v2

**v1's thesis**, restated with one sharper edge:

> CredMesh is the **Statistical Bulletin No. 1** of the on-chain credit registry.
> Not a Swiss-design-school dashboard. A *bulletin*. With issue numbers, footnotes,
> errata, engrossed resolutions, registry indices, and date stamps. The discipline
> is the bulletin form, not Helvetica.

The risk every "Swiss" concept runs into is collapsing to "clean grid + Helvetica + red accent" — i.e., generic. v2's defence is the apparatus that makes a Bundesbank report different from a Substack: every quantity gets a numbered footnote tied to a source row; every page is an annex with an issue number, a page-of-pages, a compiled-at slot, and a registry index of every pubkey it cites; every destructive action is rendered as an *engrossed* resolution with a sealed red border, not a modal.

Hex addresses, slot numbers, and ed25519 signatures are first-class data, posted as *citations*. They appear in monospace inside the prose, in tabular figures inside schedules, and resolve back through the per-annex registry index at the bottom of every page.

---

## What's in v2

| File | Purpose |
|---|---|
| `tokens.css` | Single source of truth — color, type, spacing, rule weights, status glyphs |
| `elements.html` | Annex Z — primitive showcase, every UI atom rendered once with a label |
| `dashboard-frame.html` | Annex A — LP Pool Aggregates (refines v1's LpView) |
| `agent-observatory.html` | Annex B — Counterparty Dossier (NEW) |
| `governance.html` | Annex C — Resolutions Pending |

The four HTML files share `tokens.css` — change a token there and every annex updates.

---

## Type ladder

Helvetica Neue 95 / 75 / 55 / 45, falling back to Inter when Helvetica is not licensed. **No second typeface for prose.** JetBrains Mono is the *third* face, used exclusively for hex addresses, signatures, slot numbers, program-error codes, and instruction discriminators — never for prose.

| Token | Size | Line height | UI role |
|---|---|---|---|
| `--t-foot` | 9 px | 13 | Page no., gazette stamp, registry-index tag, eyebrow caps |
| `--t-fine` | 10 px | 15 | Footnote, caption, source line |
| `--t-rule` | 11 px | 16 | Axis tick, secondary table cell, hex address |
| `--t-base` | 12 px | 17 | Body, primary table cell |
| `--t-lede` | 14 px | 20 | Lede paragraph, summary |
| `--t-head` | 18 px | 24 | Section heading (H2) |
| `--t-display-s` | 24 px | 28 | Sub-display, schedule heading, dossier name |
| `--t-display-m` | 38 px | 36 | § / Resolution number |
| `--t-display-l` | 56 px | 56 | Stat hero (TVL, Share Price) |
| `--t-display-xl` | 80 px | 72 | Annex masthead letter (A / B / C / Z) |

Weight mapping: 95 Black for masthead, all display sizes, and stat heroes. 75 Bold for headings and table column heads. 55 Roman is the default body weight. 45 Light is reserved for footnotes and captions; never used above 12 px.

Numerals are tabular everywhere a number appears — `font-variant-numeric: tabular-nums` is set on `body` and re-asserted on `.num`, `.mono`, and every `table`. Decimal points stack vertically across rows. Pubkey truncations follow the **first 4 + last 4** rule from UX_SPEC §4.5; full pubkey resolves via the registry index.

---

## Color tokens

One accent — sovereign red `#C8102E`. At most two reds per screen. Reserved for: (a) a numeric delta of consequence, (b) the destructive-action gate, (c) the registered-issuer mark in the masthead. Status is encoded by glyph + position, not by hue.

| Token | Hex | Use |
|---|---|---|
| `--paper` | `#F4F2EE` | Primary canvas (bond paper) |
| `--paper-2` | `#ECE9E2` | Sunken row, alt strip |
| `--paper-3` | `#E2DED4` | Pressed/hover state |
| `--paper-edge` | `#DEDBD3` | Off-page background (viewport) |
| `--ink` | `#0A0A0A` | Primary type, hairlines |
| `--ink-2` | `#1A1A1A` | Second-tier type |
| `--mute` | `#6B6B6B` | Labels, captions, footnotes |
| `--mute-2` | `#908A7E` | Tertiary, axis ticks |
| `--rule-fine` | `#C9C4B7` | Hairline between table rows |
| `--rule-dot` | `#A89E89` | Dotted leader (TOC, registry) |
| `--red` | `#C8102E` | Sovereign accent |
| `--red-deep` | `#8E0B20` | Engrossed-stamp variant, hover on red |
| `--red-tint` | `#F2D9DD` | Destructive panel wash |

No green. No yellow. No blue. The "OK" mark is a black `✓`; the "pending" mark is a red `○`; the "errata" mark is a red `✖`. Greenness is a fintech tell — bulletins don't wear it.

---

## Spacing scale

4 px base. Whitespace is structural — a 4-column gap is a paragraph break.

`--s-1: 4` · `--s-2: 8` · `--s-3: 12` · `--s-4: 16` · `--s-5: 24 (gutter)` · `--s-6: 32` · `--s-7: 48 (§-break)` · `--s-8: 64`

Twelve-column grid, 24 px gutters, 40 px outer page margin, 1 360 px page width with a 1 px ink border and a printer's-mark registration tick at top-left.

---

## Rule weights

Three rule weights only. Don't invent more.

- `--rule-hair` 0.5 px — between rows, dividing cells inside a stat strip
- `--rule-thin` 1 px — section heading, schedule top/bottom, action panels
- `--rule-heavy` 1.5 px — masthead break, voting-power card, schedule top
- `.heavyrule` — masthead's bulletin double rule (1.5 px + gap + 0.5 px)
- dotted hairline `0.5 px dotted var(--rule-dot)` — TOC leaders, registry-index rows

---

## Concept-unique primitives

These are the shapes 09 and 04 don't draw. Each lives in `tokens.css` and appears at full size in `elements.html`.

### 1. Annex masthead
Top-of-page slab carrying a single huge letter (`A`, `B`, `C`, `Z`) plus the dossier name and a date/slot stamp. Identifies which bulletin annex you're reading.

### 2. Footnote-binding
Every quantity carries a numbered superscript — `<sup class="fn-ref">¹</sup>`. Each section ends with a 2-column `.footnotes` block resolving each ref to a source line. Reading a number is a two-step: notice the ¹, glance below for the source. The convention is the bulletin's most distinctive idiom.

### 3. Registry index
Per-annex table at the bottom of every page resolving every short pubkey shown above to its full 32-byte form. Lets us put `[A-1]` and `5tQp…m9wK` side by side in dense tables without losing the canonical address. Indexes are scoped to the annex (`[A-1]` may differ between Annex A and Annex C).

### 4. Lifecycle ledger
Three-row gazetted entry for every transaction: `▸ SENT`, `○ PENDING`, `✓ FINALIZED`. Each row shows the state, a description, the truncated tx-sig, and a slot stamp. Failure replaces the third row with a red `.errata` block carrying the program-error code + name.

### 5. Engrossed action panel
Destructive-action chrome. Heavy red border, red wash, an "ENGROSSED" tag-stamp graphic, and a footrule containing the irrevocable-action footnote in red italic. This is the concept's destructive-confirm gate, replacing modal dialogs entirely.

### 6. Errata block
Red-bordered compact list of failed transactions, each with the `0x` program-error hex, the typed `CredmeshError::*` name, and the slot stamp. Surfaces below the recent-settlements table.

### 7. Resolution card
Three-column governance unit: `param-diff` (old → new in display-mono), vote tally bars, and a timelock countdown box. Status pill: `Active vote` (ink), `In timelock` (red), `Engrossed` (paper).

### 8. Stamp glyphs
`✓ ○ ▸ † ✖` are the canonical state marks. Used inline (`<span class="mk ok"></span>`), in tables, and on pills. They replace coloured chips — bulletins encode by glyph and position, not by colour.

### 9. Step chart
Reputation digest is a discrete on-chain commit, not a continuous quantity. The chart's polyline is a step path (horizontal then vertical), each transition stamped with its `+N` magnitude under the tick.

### 10. Compile-at colophon
Bottom of every page: `Compiled DD.MM.YYYY · HH:MM:SS UTC · slot N · block 9F3a…71B2 · programme cREdM…sH11`. A bulletin shouldn't be print-able without its date stamp; neither should its dashboard.

---

## Agent-vs-human treatment (UX_SPEC §4)

| Rule | How v2 honours it |
|---|---|
| Hex addresses are first-class | `.pk` chip in dense tables shows `[A-1]` + `5tQp…m9wK` together; full key in registry index. Truncations are 4+4. |
| Two units of time | Every tx row shows both human-relative ("2 m 41 s ago") AND `slot 268 437 902`. |
| Solana errors get program-error names | Errata rows show both `0x1771` and `CredmeshError::InvalidFeeCurve`. |
| Tx lifecycle visible | The `.ledger` primitive renders Sent → Pending → Finalized as three rows, each with a slot stamp. Never collapsed. |
| Pubkey truncation | First-4 + last-4 by convention; full on copy/hover and via the registry index. |
| Numbers carry units | `USDC`, `pp`, `%`, `slot`, `BPS`, `SHARE` — every value labelled. |
| Destructive actions get concept chrome | `.action-panel.is-destructive` — red border, red wash, "ENGROSSED" tag, red-italic footrule. No bare modals. |

---

## What v2 sharpens versus v1

1. **5-cell hero strip** (was 3-cell). TVL · Share Price · Utilisation · Active Agents · Pool Status. Pool status is a two-state register (`Open ↔ Frozen`) — a binary that needs to live above the fold.
2. **Yield attribution** — the new `Table 2.B` and stacked attribution bar reconcile the day's share-price ∆ to its on-chain sources, with the redest segment carrying "Other" so it earns the accent.
3. **Lifecycle ledger** is now a primitive (`.ledger`), not a one-off. Sent → Pending → Finalized is the pattern across deposit, withdraw, advance, and any future user action.
4. **Errata block** added under settlements. Failed transactions surface with their typed error name; `0x1771` becomes `CredmeshError::InvalidFeeCurve`.
5. **Registry index** is now per-page. Every short pubkey resolves through `[A-N]` / `[P-N]` / `[O-N]` tags in a dotted-rule table at the foot of the annex.
6. **Annex masthead** introduced — Annex B and C now open with a 80 px letter and dossier name, locking the bulletin metaphor.
7. **Engrossed-stamp chrome** for destructive actions replaces v1's implicit "are you sure?". Withdraw, sweep, and emergency exit all gate through it.
8. **Annex bar** replaces the v1 TOC. It functions as both site-nav and bulletin-pagination ("Annex A · A.0").

---

## Anti-rules (kept from v1, restated for v2)

- No purple. No glassmorphism. No 3-D. No gradients. No emoji. No fake AI mascots.
- No rounded corners. No shadows. Hairlines only.
- No animation. Documents don't animate. (Hover state = underline only; no transitions.)
- No icons drawn from a UI library. Only the bulletin's own stamp glyphs (`✓ ○ ▸ † ✖`).
- No Connect-Wallet button. The wallet is part of the page chrome (the registered-issuer mark sits next to the masthead, not inside a CTA).
- No charts where a table will do. When a chart and a table show the same data, the table is canonical and the chart is annotated as "derived" in its caption.

---

## Designer handoff notes

- All page widths are 1 360 px. The 1 440 × 900 viewport is treated as the canonical inspection size.
- Hairlines are 0.5 px; verify they survive at print 100 %. If they vanish at 1× DPR, bump to 0.75 px on the offending element only.
- Tabular figures are mandatory for any column showing numbers. If a column drifts off-grid, check `font-variant-numeric` is inherited.
- The `[A-N]` registry tags are advisory only — they exist so a designer can build a dense schedule without losing the canonical pubkey. Truncated `5tQp…m9wK` is the human-facing form; full pubkey comes from copy or registry.
- `tokens.css` is the single source of truth. If a value isn't in tokens, it doesn't belong on the page.
