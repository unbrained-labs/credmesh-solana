---
concept: 02-industrial-throughput
designer: agent-02
---

# CredMesh — Industrial Throughput

## Thesis

Credit is **fluid**. It has volume, pressure, flow rate, and a relief valve set
to a specific PSI. CredMesh, in this world, is a process plant: liquidity is
feedstock, the pool is a pressurized vessel, agents are tap-offs on a manifold,
reputation is the gauge welded to the line that tells you whether to open the
valve another quarter turn.

The brand-world is the **1968 Honeywell control room** rendered for the
machine-credit era — P&ID schematic on cream vellum, mechanical totalizers
turning behind cracked glass, brass bezels, fume-yellow alarm tags, the
Soviet/Apollo flavor of typography that exists because information had to
survive a centrifuge. Nothing glows. Nothing springs. A number changes by
clacking over.

What a human reader should feel is **weight**. This is not a javascript app;
it is a piece of infrastructure with a serial number stamped on the chassis.
When an agent opens an advance, a valve opens. There is a sound.

## Design rules

1. **Typography is IBM Plex Mono. Only.** No second face. Scale + weight + tracking
   carry every hierarchy.
2. **No circles as primary shapes.** Gauges are circles because they have to be;
   nothing else gets to be one. Cards are rectangles, ideally with a 1px stroke
   and a corner-cut callout.
3. **Numbers are mechanical.** Every quantity reads in tabular figures with a
   leading zero pad and a unit suffix in 70%-opacity caps. `0034.7821 BBL/S`,
   not `34.78 USDC`.
4. **Color is industrial, not digital.** Brass `#B8965A`, oxidized iron `#3A3530`,
   bone `#EDE6D3`, fume yellow `#E8C547`, alarm red `#C2362B`. No blue. No purple.
   No glow. No gradients except the brushed-metal kind, used once.
5. **Every line has a weight reason.** 0.5px = schematic line. 1px = vessel wall.
   2px = primary flow. Lines never get arbitrary widths.
6. **Charts are flow rates and pressure curves.** Utilization is drawn as a
   manometer arc, not a donut. History is drawn as a strip-chart with a paper
   feed direction. Never a smooth d3 area chart.
7. **Annotations are flange callouts.** Labels live on a leader line with a
   filled circle terminator and a tag bubble — the way a real P&ID labels a valve.
8. **The grid is visible.** A faint 8px crosshatch sits under everything, the way
   engineering vellum has a printed grid. UI sits *on* the paper.
9. **Tag everything with an instrument code.** `FT-204` (flow transmitter),
   `PI-117` (pressure indicator), `LIC-001` (level indicator-controller). The
   protocol speaks process-control.
10. **No marketing voice.** Status copy is a maintenance log. "VALVE OPEN. FLOW
    NOMINAL. NEXT INSPECTION 06:00 UTC." If it sounds like a SaaS landing page,
    delete it.

## Tagline

**CredMesh — Throughput, gauged.**

(alt: *"On-chain credit, plumbed."*)
