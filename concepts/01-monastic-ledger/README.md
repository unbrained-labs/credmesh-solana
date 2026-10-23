# 01 — MONASTIC LEDGER

> *Liber Machinarum* — the ledger of machines.

## Brand-world thesis

Credit, in this world, is not a financial product. It is a **canonical record of obligation** kept in a book as old as commerce itself. CredMesh is rendered as a working scriptorium — a Cistercian house keeping the accounts of agents who cannot keep their own. The protocol is not a "platform"; it is the *liber*, the bound volume in which every advance, every receivable, every reputation cycle is inscribed in ink and witnessed by the order.

The aesthetic is anti-fintech to the point of austerity. Stripe is glass; Aave is neon. We are vellum, lamp-black ink, and a single sparing rubric. A CredMesh dashboard should feel less like a trading terminal and more like a 1740 Bank of England drawing-office ledger that happens to record the obligations of autonomous machines. The reader should be aware that the *form* is older than any of the parties to it — that an agent borrowing against a verifiable receivable is participating in something with continuity to Domesday, to the Cistercian cartularies, to the merchant houses of Venice. The book has always been here. We are merely a new chapter.

Credit, here, **feels weighty**. Numbers are illuminated because numbers are sacred. Tables are dense because vellum is expensive. The page is not "designed" — it is **set**, in the printer's sense.

## Design rules

1. **Type first, type only.** EB Garamond (or Adobe Caslon Pro / Adobe Garamond Pro in production) is the sole typeface family. No sans-serif. No display fonts. Italic small caps for captions; titling caps with letter-spacing for section heads.
2. **Three colors, no exceptions.** Parchment cream `#EFE6CF`, lamp-black ink `#15110A`, and a single vermillion rubric `#A8261E`. Vermillion appears ONLY for marginalia, illuminated initials, paragraph marks, and the leading digit of the largest numbers. Never for buttons. Never for chrome.
3. **The leading digit of every important number is illuminated.** Rendered ~1.4× the body figure size, in vermillion, set tight against the rest of the figure. The "$" is also vermillion.
4. **Old-style figures everywhere.** `font-feature-settings: "onum", "tnum"` for tables. Numbers must look like *prose*, not like a stock ticker.
5. **No rounded corners. Ever.** No box-shadows, no glassmorphism, no gradients. The only depth allowed is the subtle warmth of the parchment ground itself.
6. **Rules, not boxes.** Tables and sections are divided by hairlines (0.5px) and double-rules (a 1px + 0.5px pair, ~3px apart). Nothing is in a card. Nothing has a border-radius.
7. **Latin section heads with English subtitles.** `LIQUOR` over *liquidity in repose*. `USURA` over *the rate of use*. `FAMA` over *reputation*. Latin is set in titling caps with +180 tracking; English is italic small caps, half the size.
8. **Dense composition.** Multi-column. Tight leading (~1.15). Margins narrow. Captions hugged against the figure they annotate. Whitespace is rationed; the eye should *work*.
9. **No icons, no glyphs, no emoji.** Pilcrows (¶), section marks (§), em-dashes, and the long-s where typographically appropriate. If it didn't exist by 1750, it doesn't exist here.
10. **Charts are engravings, not data-viz.** The utilization curve is a hand-hatched area, drawn with vertical strokes, not a filled gradient. Sparklines are pen lines, not SVG fills.
11. **Every page is dated *Anno Domini* in Roman numerals**, with a folio number in the upper margin. The system has chronology. It remembers.
12. **The signature line.** Every primary surface has a marginalia footer in italic — a one-line comment from the scribe. This is where personality lives. Nowhere else.

## Sample tagline

> **CredMesh.** *Liber Machinarum* — a ledger of machines, kept in the manner of those who kept the first.

Alt: *"The book has always been here."*
