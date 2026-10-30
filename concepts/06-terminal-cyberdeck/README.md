# 06 — terminal/cyberdeck

> A TERMINAL FOR CREDIT YOUR MACHINES TAKE WHILE YOU SLEEP.

## Thesis

Credit, in this world, is *fuel for autonomous processes.* The product is for
the machines; the brand is for the humans who watch them transact.

So the brand is not a website. It is a **terminal session**. Phosphor amber on
black, like the original Bloomberg of 1981 — the one with the hose-cabled
keyboard that used to live in a closet and pipe data over the trading floor.
But that terminal escaped, was rehoused inside a basement cyberdeck, and is
now showing live agent advances instead of T-bills. The chrome remembers its
financial-data-vendor lineage. The internals remember the soldering iron.

What credit *feels like* in this world: a meter on a power rail. An agent
opens a position the way a process spawns. A repayment is a heartbeat packet.
A default is a fault light. The TVL is a phosphor readout that ticks. You
don't "log in" — you watch the screen, and if you don't like what the
machines are doing, you trip a breaker.

Bloomberg-orange data density (the Terminal that nobody has redesigned in
thirty years because it does not need to be redesigned), routed through
hand-soldered cyberdeck materiality (silkscreen white on PCB green, copper
traces, exposed standoffs, an F1–F12 row that's actually wired). Hex
addresses are the typography of record. The protocol does not need a logo
— machines read program-IDs, and humans read the wordmark.

## Design rules

1. **Monospace or nothing.** JetBrains Mono for data, VT323 for hero
   readouts. Never proportional. Never sans-serif body text.
2. **Amber `#FFB000` on black `#000000`** is the only chrome palette.
   Phosphor green `#4FFF7A` signals `ALIVE` only. Red `#FF3344` signals
   `DEFAULT` only. Copper `#B87333` belongs to the logo and the silkscreen
   layer. No purples. No gradients (except the CRT vignette).
3. **Box-drawing characters (┃ ━ ╋ ┏ ┓ ╔ ═) are the layout primitives.**
   CSS borders are forbidden in the chrome. Lines exist as text glyphs, not
   strokes.
4. **Charts are ASCII.** Bar charts use block elements `▁▂▃▄▅▆▇█`. Line
   charts use box-drawing glyphs `─ ╭ ╮ ╯ ╰`. Bezier curves are forbidden.
   The only SVG on the page is the logo and corner fiducials.
5. **Every number is right-aligned. Decimals align to the period.** The
   currency suffix sits in a fixed column. Tabular numerals on, always.
6. **Hex addresses are first-class typography.** Show as `0x4F2c…aA91`.
   Never linkified with an underline, never wrapped. They are part of the
   texture, not metadata.
7. **The cursor blinks at 1 Hz.** One `█` block at the active prompt. The
   cursor is the only animated element on the page.
8. **The bottom row is a fixed F1–F12 function-key bar.** It is not
   navigation — it is a UI primitive. Even when nothing is bound, it stays.
9. **No rounded corners. No drop shadows. No glassmorphism.** The CRT glow
   (`text-shadow`) is the only depth effect, and it lives on text, never
   on containers.
10. **The logo appears once, on splash.** Everywhere else the brand is the
    wordmark `CRED.MESH/v1` in caps. Humans watch machines transact; the
    brand is a label, not a sigil.

## Voice

Telegraphic. Verbs imperative. No marketing adjectives. Numbers and
addresses do the work. Headers are commands (`POOL.QUERY`, `ADV.LIST`,
`AGENT.RANK`). Nothing ends in an exclamation. Status messages are five
words or fewer. The terminal does not advertise itself.

## Tagline

> **A TERMINAL FOR CREDIT YOUR MACHINES TAKE WHILE YOU SLEEP.**

Alternates considered:
- `CRED.MESH // PROTOCOL UPLINK ESTABLISHED.`
- `FUEL FOR AUTONOMOUS PROCESSES.`
- `THE BLOOMBERG ON THE OTHER END OF YOUR AGENT'S WALLET.`
