# Drape benchmark log

`cargo run --release -p opendrape-testkit --example drape_bench -- [--threads 1] [--scene …]`
prints, per scene, the wall-clock milliseconds per simulated frame, where the time went
(the solver's phase timers), and the drape's quality: penetration, strain p99, how many
fabric edges pass through fabric (`crossings`, 0 is clean) and how creased the welded seams
are (`seam crease`, the mean angle between the triangles either side of a seam edge; a seam
that lies flat round a 10 cm curve reads about 7°).

`--threads 1` stands in for a slow laptop: a 2-core machine has one worker for the
parallel body query, and its single-thread speed is about a third of the M4 Max's.

Each scene runs 4 simulated seconds (240 frames). Numbers are the mean over those frames,
on an Apple M4 Max (14 cores), release build, nothing else running.

## Phase 0 — baseline (2026-10-11, before any solver change)

| Scene | Particles | ms/frame (14 threads) | ms/frame (1 thread) | Penetration | Strain p99 | Crossings | Seam crease |
|---|---|---|---|---|---|---|---|
| skirt (demo grid) | 4,700 | 14.0 | 16.7 | 0.00 mm | 7.5 % | 0 | 17.3° |
| tube (demo grid) | 1,440 | 4.4 | 4.8 | 0.00 mm | 3.1 % | 0 | — |
| drafted-skirt | 5,668 | 16.6 | 20.1 | 0.00 mm | 6.4 % | 0 | 16.7° |
| drafted-tshirt | 7,407 | 21.7 | 27.7 | 0.00 mm | 4.5 % | 12 | 59.5° |

Where the time goes (ms per frame, 1 thread):

| Scene | body query | predict | bend | stretch | collide | velocity |
|---|---|---|---|---|---|---|
| skirt | 3.28 | 0.21 | 3.38 | 9.43 | 0.19 | 0.14 |
| tube | 0.70 | 0.06 | 1.03 | 2.93 | 0.03 | 0.04 |
| drafted-skirt | 4.31 | 0.25 | 2.82 | 12.30 | 0.22 | 0.17 |
| drafted-tshirt | 6.97 | 0.33 | 3.70 | 16.20 | 0.22 | 0.22 |

What the baseline says:

- The stretch pass is 55–75 % of every frame and the bend pass another 15–25 %. Both are
  serial Gauss–Seidel over every link, 40 times a frame (20 substeps × 2 iterations): about
  17 ns per link solve, which is memory-bound random access in f64.
- The body query (parry closest point for every particle, once a frame) is about 1 µs per
  particle on one thread: 7 ms of the T-shirt's 28 ms. With 14 threads it is 1 ms. On a
  2-core laptop it stays near the 1-thread figure.
- The T-shirt's seams are creased at 59° on average and its fabric crosses itself 12 times
  (the sleeves through the bodice): the two things the next phases fix.
