# Inspiration · 03 — Mycelium / Organism

References I actually used. Each is cited with what was lifted.

## Primary plates

1. **Ernst Haeckel — *Kunstformen der Natur*, Tafel 63: Basimycetes (1900)**
   http://www.biolib.de/haeckel/kunstformen/high/Tafel_063_300.html
   The single biggest visual reference. I lifted the *plate frame* (a thin double-rule rectangle with the title bottom-centered and the tafel number top-right), the *radial composition* of multiple specimens around a dominant central form, and the *naming convention* — a comma-separated list of Latin binomials below the plate. Screenshot saved at `inspiration/screenshot-01-haeckel-basimycetes.png`. The dashboard's central pool specimen is a direct homage to the central *Clathrus* in this plate.

2. **Ernst Haeckel — *Kunstformen der Natur*, full archive (Internet Archive, 300dpi scans)**
   https://archive.org/details/KunstformenDerNaturErnstHaeckel
   For the *line-engraving feel*: stippled spores, cross-hatched shading following the curve of the form, terminal sporangia drawn as small filled circles. The sporangia at the cap fringe in `dashboard-frame.html` are pulled directly from the convention.

3. **Mordecai C. Cooke — *Edible & Poisonous Mushrooms* (1894), reproduced at MykoWeb**
   https://www.mykoweb.com/OldBooks/Edible_Poisonous.html
   Lifted the *page rhythm* of a Victorian field guide: title plate, plate number in roman, descriptive text in two columns, and the slightly desaturated parchment color rather than pure white. Screenshot saved at `inspiration/screenshot-02-cooke-plates.png`.

4. **Aurel Dermek — *The Spotter's Guide to Mushrooms and Other Fungi* (Fonts In Use)**
   https://fontsinuse.com/uses/43953/the-spotters-guide-to-mushrooms-and-other-fun
   Validated the *Windsor Elongated + Optima* combination of a 19th-century display serif paired with a clean humanist typeface — translated here to Cormorant Garamond (display) + Source Serif 4 (body), keeping the same character: ornate-headline, calm-body.

## Books that shaped the brand-world thesis

5. **Merlin Sheldrake — *Entangled Life: How Fungi Make Our Worlds, Change Our Minds & Shape Our Futures* (2020)**
   https://www.merlinsheldrake.com/entangled-life
   The conceptual frame. Sheldrake's "wood-wide web" — sugar routed through mycorrhizal networks between trees that have never met — *is* the credit-protocol metaphor. The line "Credit is what the forest is doing underground" is a direct paraphrase of his thesis that exchange precedes the things being exchanged.

6. **Linnaeus — *Systema Naturae* (1758, 10th ed.)**
   https://en.wikipedia.org/wiki/Systema_Naturae
   The *binomial naming convention* used everywhere in the dashboard ("Aspergillus — ferri", "Coprinus — lucens"). Genus + epithet = a recognizable structure that signals "classified, catalogued, part of a system" without needing translation. This is what makes the Latin labels feel grave rather than precious.

7. **Royal Botanic Gardens, Kew — Herbarium specimen sheets**
   https://www.kew.org/science/collections-and-resources/collections/herbarium
   Lifted the *physical layout of a herbarium sheet*: a printed gridded margin, a specimen number top-right ("Tab. CCXIV"), generous whitespace, and the convention that the specimen sits centered on the sheet with annotation tics radiating outward. The HTML page is a literal herbarium sheet.

## Typographic & ledger references

8. **Bowyer Ledgers (1710–1781), Grolier Club archives**
   https://www.grolierclub.org/default.aspx?p=v35ListDocument&ID=755370869
   The *tabular ledger feel* of the Recent Advances table: hairline rules only, italic column headers, old-style figures (oldstyle numerals are the default in 18th-c. printed ledgers), Roman numerals for the leftmost rank column. No vertical rules — separation comes from baseline rhythm and gutter alone.

9. **William Townsend & Sons private trade ledger (1885–1915), RIT digital collections**
   https://digitalcollections.rit.edu/luna/servlet/detail/RIT~1~1~362~3609
   Confirmed the convention of *Roman numerals for sequence and Arabic for value* — exactly what the dashboard does (rank in Roman: i. ii. iii.; amounts in Arabic with old-style figures).

10. **Marginalia (H. J. Jackson, 2001) — readers writing in books**
    https://thefeeledlab.ca/wp-content/uploads/2022/10/book-jackson-2001-marginalia-readers-writing-in-books.pdf
    Justified the small italic *marginalia* in the corner of the sheet ("[ specimen sheet · printed at MMXXVI · pp. xxiv ]") — the convention that a real reader's book has tiny notes in the corners is what makes the page feel inhabited rather than designed.

## What I deliberately did not look at

- Any Solana/Aave/Drift/Jupiter dashboard. None.
- Any "biotech" website (the Calico/Apeel aesthetic of soft white + green). That's the hostile mimic of this concept; this is the real thing.
- Any AI-generated mushroom illustration. The whole point is that this is hand-drawn by a human who has read Sheldrake.

## Screenshots in this folder

- `screenshot-01-haeckel-basimycetes.png` — the core plate reference.
- `screenshot-02-cooke-plates.png` — Victorian field-guide page rhythm.
- `screenshot-03-dashboard-preview.png` — first render of `dashboard-frame.html` at 1440x900.
- `screenshot-04-dashboard-fixed.png` — final viewport screenshot after fixing label collision.
