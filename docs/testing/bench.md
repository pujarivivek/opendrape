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

## Phase 1 — seams weld flat, each when it has closed (2026-10-11)

A hinge across a welded seam now rests where the pattern lays flat (unfolded from rest edge
lengths), not where the fabric happened to be as it welded; each seam welds on its own once
every stitch of it is within 4 mm, and a seam that can't close is pulled shut at 3 s with a
note.

| Scene | Particles | ms/frame (14 threads) | ms/frame (1 thread) | Penetration | Strain p99 | Crossings | Seam crease |
|---|---|---|---|---|---|---|---|
| skirt (demo grid) | 4,700 | 12.7 | 15.1 | 0.00 mm | 7.6 % | 0 | 8.9° (was 17.3°) |
| tube (demo grid) | 1,440 | 3.9 | 4.2 | 0.00 mm | 3.1 % | 0 | — |
| drafted-skirt | 5,668 | 15.1 | 18.0 | 0.00 mm | 6.6 % | 0 | 9.2° (was 16.7°) |
| drafted-tshirt | 7,407 | 19.6 | 24.9 | 0.00 mm | 4.8 % | 28 (was 12) | 28.9° (was 59.5°) |

- The skirts' seams now read 9°, about the floor for a flat seam round a 10 cm curve. The
  T-shirt's halve; what is left is the sleeve seams, where a sleeve tube really does meet the
  bodice at an angle, and the sleeves still passing through the bodice.
- Crossings rise on the T-shirt because flatter seams push the sleeve further into the
  bodice, which nothing stops yet: Phase 3's job.
- The timings are within run-to-run noise of the baseline (the solver's work is unchanged);
  about ±10 % between runs on this machine.

## Phase 2 — fabric on the grain (2026-10-11)

Each panel is filled with a lattice of points on its grainline (rows along the warp, the
spacing the edge length), refined only in the band where the lattice meets the outline.
Edges along the warp or weft are structural and hold their length; the cells' diagonals, and
the hinges across them, are the bias and shear softly (`Params::shear_compliance`, 0.05).
The strain p99 is now the structural edges'; the bias has its own figure. The demo grids
(skirt, tube) have no grain, so they are unchanged.

| Scene | Particles | ms/frame (14 threads) | ms/frame (1 thread) | Penetration | Strain p99 | Bias p99 | Crossings | Seam crease |
|---|---|---|---|---|---|---|---|---|
| skirt (demo grid) | 4,700 | 12.9 | 15.5 | 0.00 mm | 7.6 % | — | 0 | 8.9° |
| tube (demo grid) | 1,440 | 3.9 | 4.3 | 0.00 mm | 3.1 % | — | 0 | — |
| drafted-skirt | 3,868 (was 5,668) | 7.2 (was 15.1) | 9.5 (was 18.0) | 0.00 mm | 4.8 % (was 6.6 %) | 7.6 % | 0 | 11.4° |
| drafted-tshirt | 5,050 (was 7,407) | 9.2 (was 19.6) | 12.8 (was 24.9) | 0.00 mm | 3.4 % (was 4.8 %) | 6.4 % | 8 (was 28) | 26.6° |

- A square lattice at 12 mm has about 0.6 of the points a refined near-equilateral mesh has,
  and uniform spacing, so the drafted garments run about twice as fast at the same edge
  length, and the cache-friendly row order helps the link solve too (the T-shirt's stretch
  pass: 14.4 → 5.1 ms for 0.68 × the particles).
- Structural strain falls because the bias gives (6–8 % at p99 on the diagonals), as fabric
  does; fewer crossings for the same reason.
- The skirts' crease reads 2° higher: a lattice's diagonal hinges are longer than a
  refined mesh's, so the same curve bends in slightly bigger steps. The floor for a flat seam
  on a curve is now about 9–11°.

## Phase 3 — cloth keeps off cloth (2026-10-11)

Every particle keeps half an edge length off every triangle it is not part of, from the
moment the garment's seams have all welded. Pairs are found through a spatial hash (from
each particle to the triangles its hashed neighbours are corners of) and found again only
when something has moved more than a quarter of an edge length; each pair remembers which
side of the triangle the particle was on, so a particle that drifts across between two looks
is put back. Across a seam, each stitched pair and the neighbours of its ends are left alone.

| Scene | Particles | ms/frame (14 threads) | ms/frame (1 thread) | of which self-collision | Penetration | Strain p99 | Crossings | Seam crease |
|---|---|---|---|---|---|---|---|---|
| skirt (demo grid) | 4,700 | 16.7 | 19.3 | 3.3 | 0.00 mm | 7.6 % | 0 | 8.9° |
| tube (demo grid) | 1,440 | 4.1 | 4.5 | 0.2 | 0.00 mm | 3.1 % | 0 | — |
| drafted-skirt | 3,868 | 10.6 | 13.0 | 2.9 | 0.00 mm | 4.8 % | 0 | 11.4° |
| drafted-tshirt | 5,050 | 11.8 | 15.4 | 2.1 | 0.00 mm | 3.4 % | 24 | 26.6° |

- `tests/layers.rs`: a square dropped on one held level by its corners is caught 3 cm above
  it (the held one sags) with no crossings; without self-collision it falls through to the
  floor.
- What self-collision costs is almost all finding the pairs again: about 2.5 ms each time for
  4–5k particles, once or twice a frame while the cloth is moving, and hardly ever once it has
  settled. Particle-to-particle pushing alone (tried first) was cheaper but left shallow pokes
  wherever a particle sat over the middle of another layer's cell, where the four corner
  pushes cancel.
- The T-shirt's 24 crossings are one patch about 3 cm across at the top of a sleeve cap, at
  the back, where three seams meet at a saddle: the fabric folds through itself while the
  seams pull shut, before self-collision begins. With self-collision on from the start the
  finished T-shirt has no crossings at all, but no seam closes by itself: particles held off
  the other panel cannot reach the stitch, and all of them shut only at the 3 s timeout, with
  a note each. So seams close first. An arrangement that keeps the pieces from needing to pass
  through each other on the way shut would get both.
