# CredMesh — UX Spec for v2 Brand Refinement

**Audience for this doc:** the 3 designers refining concepts 04 / 09 / 10. Read this first; everything you build references it.

**Goal of v2:** convert each concept's brand world into a *designer-ready handoff* — type ladder, color tokens, spacing scale, surface inventory, and the actual UI for the new surfaces nobody has drawn yet. Original v1 outputs stay untouched; v2 sits in `concepts/NN-name/v2/`.

---

## 1. Audiences (real users, not personas)

| Audience | Sees | Cares about | Surface |
|---|---|---|---|
| **Liquidity provider** (human) | money in / out, share price drift, where the yield came from | preserving capital; transparent fee curve | LP Dashboard |
| **Governance voter** (human, often LP-overlap) | active proposals, parameter deltas, timelock countdown, vote tally | their LP capital not being rugged via bad params | Governance |
| **Agent operator** (human running ≥1 AI agent) | what their agent did, what it owes, what its reputation is, where it borrowed | agent solvency, debug surface when it misbehaves | Agent Observatory |
| **Agent** (machine) | program interface only — instruction discriminators, account layouts, error codes | atomic correctness; deterministic state | Program ABI (no UI) |
| **Ops watcher** (protocol-side human) | TVL, default rate, oracle health, queue depth, key-rotation status | systemic health | Public Stats |

**Critical insight:** agents don't browse dashboards. Their "UX" is the program interface. The thing called "Agent UI" is really an *observatory* for the **human watching one specific agent's machine behavior** — debugging tool, audit trail, kill switch.

---

## 2. Surfaces

Five surfaces. Each concept must produce three of them in v2. The other two are noted for later.

### 2.1 LP Dashboard (refining v1's LpView)

**Hero strip:** TVL · Share Price (4 dp) · Utilization · Active Agents · Pool Open/Frozen
**Time-series:** share price, last 30d. Hover tooltip shows slot + timestamp + delta.
**Action panel:** Deposit / Withdraw. Each is a 3-step lifecycle (review → sign → confirmed) with a slot+sig receipt.
**Recent settlements table** (8 rows): agent · receivable id · principal · fee · slot · status. Status renders in concept-specific way (overprint stamp / dimension callout / footnote-ref).
**Yield attribution:** "where this share-price uptick came from" — micro-table breaking down the last day's settlements by source (utilization fees · liquidation surplus · receivable interest).

### 2.2 Agent Observatory (NEW — does not exist in v1)

The page a human looks at to know what their agent is doing. Dense.

**Identity strip:** Squads vault pubkey (truncated + copy), MPL Agent registry pubkey, owner pubkey, "operator since" slot.
**Vitals card:** outstanding debt USDC · current reputation digest (16 hex chars + delta arrow) · advances active · last on-chain activity (time-since + slot).
**Reputation timeline:** rolling-digest movement over 30d as a chart. Each digest transition is a discrete step (not interpolated).
**Recent advances table** (10 rows): advance id · pool · principal · fee accrued · receivable status · slot opened · slot settled (or "open"). Hex addresses are first-class.
**Receivable inflow log** (10 rows): receivable id · source kind · payer pubkey · ed25519 sig (truncated) · slot · "consumed yes/no". Designed for grep-by-eye.
**Health panel:** oracle staleness · key rotation due in N days · idle USDC sweepable · circuit breakers tripped.
**Action panel:** "Manual repay" · "Withdraw idle USDC" · "Request emergency exit" — destructive actions get the concept's strongest visual chrome (stamp / dimension callout / footnote with red).

### 2.3 Governance (refining v1's GovernanceView)

**Proposals list:** active first, then pending timelock, then historical. Each proposal card shows: param being changed · old → new · proposer · timelock end (countdown) · vote tally.
**Parameter explorer:** read-only browser of current pool params (fee curve points, max utilization, reputation thresholds). Click to history.
**My voting power card:** based on LP shares × time-weight. Clear "you voted X on proposal Y; not yet executed" status.
**History:** last 20 executed proposals with diff shown.

### 2.4 Public Stats (NOT required in v2; document only)

Aggregate TVL across pools, top 10 agents by reputation (anonymized via pubkey truncation), default rate 30d, oracle uptime. Read-only, no auth.

### 2.5 Marketing landing (NOT required in v2; document only)

Public web page. Concept-driven hero, 3-section explainer, link to docs.

---

## 3. UI element library (concept-agnostic primitives)

Each concept must define HOW these primitives look in its world.

| Primitive | Purpose | Density | Notes |
|---|---|---|---|
| **Identity strip** | pubkey + role + status | high | hex first-class; copy button |
| **Stat card** | one number + unit + delta | low | the number is bigger than the label |
| **Time-series chart** | trend over time | medium | hover shows slot + timestamp + value |
| **Lifecycle timeline** | tx state transitions | medium | vertical or horizontal; concept choice |
| **Recent activity table** | event log | high | tabular figures, monospace addresses |
| **Action panel** | primary CTA + secondary | low | destructive actions visually weighted |
| **Footnote / callout** | annotation, citation | varies | concept-specific (footnote ref / margin note / overstamp) |
| **Status pill** | binary or N-state indicator | dense | concept renders this distinctively |

---

## 4. Agent-vs-human design rules

1. **Hex addresses, slot numbers, signatures are first-class data**, not metadata. They get tabular monospace, full visibility, click-to-copy. They are NOT shrunken to "advanced details."
2. **Time has two units: human and on-chain.** Show "2m ago" AND "slot 268,403,221". Both. Always. Every transaction-related row.
3. **Solana errors get program-error names, not just numbers.** `0x1771` → `CredmeshError::InvalidFeeCurve`. The error namespace is part of the brand surface.
4. **Tx confirmation lifecycle is visible.** "Sent → Pending → Finalized" — three states, with the slot at each. Don't collapse to a single "confirmed" checkmark.
5. **Pubkey truncation rule:** show first 4 + last 4 chars by default (`5tQp…m9wK`), full on hover/copy. Never show only one end.
6. **Numbers carry units.** USDC, lamports, BPS, slot, % — every quantity. Naked numbers are forbidden.
7. **Destructive actions require a concept-specific gate.** No bare "are you sure?" modals — gate them in the concept's chrome (e.g. 04: 240pt red `IRREVERSIBLE`; 09: stamped `CHECKED ☐ APPROVED ☐` rubric; 10: footnote `¹ This action posts a transaction to the canonical ledger and cannot be reverted.`).

---

## 5. Type ladder (concept-defined)

Each concept must publish:
- The fixed type scale (e.g., `11 / 16 / 24 / 36 / 54 / 80 / 120 / 180 / 240` for 04)
- The mapping: which size = section heading, which = stat hero, which = body, which = micro-copy
- Numeric vs prose treatment (tabular figures, mono vs sans, etc.)

## 6. Color tokens (concept-defined)

Each concept must publish CSS custom properties:
```css
:root {
  --paper: #...;
  --ink: #...;
  --accent-1: #...;
  --warning: #...;
  /* ... */
}
```

## 7. Spacing scale (concept-defined)

8px base or 4px base — concept choice. Publish: `--s-1, --s-2, ..., --s-8`. Use them in the v2 HTML.

---

## 8. Deliverables (per concept)

In `concepts/NN-name/v2/`:

1. **`README.md`** — refined design system: explicit type ladder, color tokens, spacing scale, list of which primitives the concept defines uniquely. Update the v1 thesis into design-system language.
2. **`tokens.css`** — actual `:root { --... }` declarations. Other v2 HTML files import this.
3. **`dashboard-frame.html`** — refined LP dashboard implementing UX_SPEC §2.1.
4. **`agent-observatory.html`** — NEW. UX_SPEC §2.2.
5. **`governance.html`** — UX_SPEC §2.3.
6. **`elements.html`** — primitive showcase (every UI primitive from §3 rendered once with a label). Useful for handoff.

All HTML files self-contained (inline CSS via `tokens.css` link if you want, or full inline). Mock data realistic — invent plausible Solana pubkeys, slot numbers in the 268M range (current devnet), USDC amounts, reputation digests as 16-hex strings.

## 9. What "refining" means

- **04 typography-cinematic:** prove the wordmark-only stance survives at info-dense agent observatory. The risk: 240pt headers don't fit when the page is a transaction log. The solution: show the type-scale earning its keep across `11/16/24/36/54/80/120/180/240` — the hero is 240, the row data is 11, the contrast is the brand.
- **09 architectural-blueprint:** prove cyan blueprint paper isn't fatiguing at full app scale. Solution: maybe the canvas is paper-cream and the blueprint cyan only appears as a "drawing layer" — sheet titles, dimension lines, callouts. The data sits ON the cream. The blueprint is the chrome, not the background.
- **10 swiss-institutional:** prove this isn't generic Swiss design school. Solution: lean HARD into "Statistical Bulletin No. 1" framing. Footnotes everywhere. Issue numbers. Date stamps. Edition revisions. This is what separates a Bundesbank report from a Substack. The discipline is more than Helvetica.

## 10. Anti-rules (apply to all 3)

- No purple. No glassmorphism. No 3D depth. No emoji. No "Connect Wallet" in a branded button — the wallet is part of the page chrome, not a feature.
- No fake AI mascots. No characters. No avatars.
- The dashboard is NOT a marketing page. It is the working surface of a credit protocol. Every element earns its place.
