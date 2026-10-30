# Inspiration · 01 — monastic-ledger

References actually consulted while building this concept. Each entry: URL · what I took from it.

---

### Manuscripts and cartularies

1. **Open Domesday — full digital folios of the 1086 survey**
   https://opendomesday.org/
   Took the dense double-column page composition, the *breves* pattern (each landowner = a discrete tabulated record), and the practice of running rubric titles in the upper margin. Our "f. xlvii r." folio mark and the per-cycle epoch numbering descend directly from this.

2. **The National Archives — Great Domesday Book entry**
   https://beta.nationalarchives.gov.uk/explore-the-collection/explore-by-time-period/medieval/domesday/
   Confirmed: "neatly written, structured text in Latin on parchment… single main scribe over three years, abbreviated Caroline minuscule, double columns, enlarged initials." That sentence is the brief.

3. **Bodleian / "Minimalist decoration in Cistercian manuscripts"** *(Heidi Hilliker on MS. Laud Misc. 144 et al.)*
   https://hab.bodleian.ox.ac.uk/en/blog/blog-post-15/
   Crucial constraint adopted wholesale: 1152 Cistercian statutes mandated *non-figurative* decoration with letters of *one colour*, "no gold or silver." This is the legal basis for our **three-colour rule** (parchment / ink / one rubric, no gold). It lets the work feel monastic-strict, not Gothic-extravagant.

4. **Balmerino Abbey Cartulary (NLS, 14th c.)**
   https://manuscripts.nls.uk/repositories/2/resources/19363
   Took the structural separation: royal/secular charters in one hand, papal/order privileges in another. We mirror the same in the dashboard split: *Praestita Recentia* (recent advances, the operating record) versus *Agentes Eminentes* (the canon of houses, a more permanent register).

5. **British Library Arundel MS 153 — Liber Domesday (imperfect)**
   https://searcharchives.bl.uk/catalog/040-002039436
   Specific lift: "alternating red and blue initials, one or two lines high, with red or blue pen-flourishing in the alternate colour. Rubrics and running titles… in red." We dropped the blue (Cistercian rule), kept the red. The "running title" pattern became our `runtitle` (`Liber CredMesh · Tomus I`).

6. **OPenn Ms. Codex 107 — Cartulary of San Andrés de Fanlo**
   https://openn.library.upenn.edu/Data/0002/html/mscodex107.html
   Confirmed the "thirteenth-century copies of older documents" pattern — our `cyclus № 218` references it: a present record that bears chronology back to its first cycle.

### Bank ledgers

7. **Bank of England · Drawing Office Customer Account Ledgers, 1694–1900 (Series C98)**
   https://www.bankofengland.co.uk/CalmView/Record.aspx?id=C98
   The format that mediated this concept from "manuscript book" to "actual financial register." Five thousand six hundred and sixty volumes; ledgers indexed A–Z; a *folio* per customer. Our recent-advances table uses this exact structure: a numbered entry, an account-holder identifier, a sum, a date, a status. Plain-bound ledger discipline.

8. **Bank of England C98/2512 — Ledger A–Z, 1694**
   https://www.bankofengland.co.uk/CalmView/Record.aspx?id=C98%2F2512
   Specific take: the first volume of the Drawing Office contains "the balances of running-cash notes issued to each customer *but also* an account of the Bank's overall operations in its first year." That's exactly our metric-strip-on-top-of-ledger composition: aggregate above, individual entries below, in one bound view.

### Typography

9. **EB Garamond (Duffner) — CTAN package**
   https://ctan.org/tex-archive/fonts/ebgaramond
   Chosen as primary face. From the docs: "revival of the 16th century fonts designed by Claude Garamont… source… Egenolff-Berner specimen, composed in 1592." That date is older than the Bank of England by a century — exactly the temporal weight we want. Old-style figures (`onum`) and tabular figures (`tnum`) are turned on globally for the ledger.

10. **Cormorant Garamond — display interpretation**
    Used only for masthead, Latin titling caps, and the illuminated metric figures. Higher contrast; reads at large sizes where EB Garamond's small x-height starts to feel thin.

11. **"Old-Style Financial Statement" — TeX StackExchange thread**
    https://tex.stackexchange.com/questions/297892/old-style-financial-statement
    Took the dotted-leader table treatment (LaTeX `\xdotfill`) and reproduced it in CSS via `border-bottom: .5px dotted` on `td` elements. It is the thing that makes the recent-advances table feel like a *bound book* and not a `<table>`.

12. **"Garamond Font Pairing" — Etienne Aubert Bonn**
    https://www.etienneaubertbonn.com/garamond-font-pairing/
    Specifically the Garamond + Caslon recommendation: "two dialects of the same typographic language… scholarly books, traditional print publishing, heritage brand identities." We took the principle (one family, two dialects) and applied it to EB Garamond + Cormorant Garamond.

### Conceptual / framing

13. **"Domesday Book" — medievalwritings.atillo.com.au** *(on Latin as ceremonial language)*
    https://medievalwritings.atillo.com.au/word/domesday2.htm
    Quote that anchored the Latin section heads: *"The work was written in Latin, the ceremonial language of law and liturgy, and laid out in double columns with enlarged initials in a neat, if heavily abbreviated, Caroline minuscule script."* CredMesh, in this concept, treats Solana program output the way the Norman administration treated taxation: as something requiring a ceremonial language. Latin headers are not nostalgia — they are the marker that this protocol is law-shaped, not product-shaped.

14. **Cartularium abbathiæ de Rievalle (1889 edition of the 12th c. Rievaulx cartulary)**
    https://archive.org/stream/cartulariumabba00atkigoog/cartulariumabba00atkigoog_djvu.txt
    Took the colophon habit. Every primary surface in our concept ends with an italic scribe's note: who inscribed, when, with what witness. This is where the brand's voice lives — nowhere else is it allowed to be informal.

---

**What I deliberately avoided:** any reference to existing DeFi UIs, any reference to "fintech minimalism" (Stripe, Mercury, Linear), any reference to crypto's neon-and-glassmorphism pattern language. The reference shelf is one bookcase in the British Library's manuscripts room. That's the world.

**Local previews generated during the build** (in this folder):
- `preview-dashboard.png` — full dashboard at 1440 × 1900
- `preview-logo.png` — logo at 32 / 120 / 320 px
