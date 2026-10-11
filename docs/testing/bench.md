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

## Phase 4 — fabric detail presets, the step budget, and a body that is asked less (2026-10-11)

`View → Fabric detail`: Draft (20 mm cells, 20 substeps of one pass, cloth against cloth
every other substep), Normal (12 mm, 30 × 1) and Fine (8 mm, 30 × 1); Auto is Draft on two
cores or fewer. The body is asked about a particle only once it has moved far enough to reach
the surface since it was last asked (or half a millimetre, in contact), so a settled drape asks
about nothing. The demo grids keep the solver's default 20 × 2 (with no bias to give, they
over-stretch at 30 × 1). `drape_bench --quality normal|draft|fine`.

The step sweep on the drafted scenes (Normal cells, 14 threads) that chose 30 × 1:

| substeps × passes | T-shirt ms | T-shirt strain p99 | skirt ms | skirt strain p99 |
|---|---|---|---|---|
| 20 × 2 (the default) | 11.7 | 3.4 % | 10.6 | 4.8 % |
| 30 × 1 | 10.0 | 2.7 % | 9.1 | 4.3 % |
| 40 × 1 | 12.3 | 2.1 % | 11.8 | 3.7 % |
| 24 × 1 | 9.0 | 3.4 % | 7.8 | 4.8 % |
| 15 × 2 | 9.5 | 4.0 % | 8.1 | 5.0 % |
| 12 × 2 | 7.2 | 5.1 % | 7.1 | 5.5 % |

The presets, on one thread (the slow-laptop stand-in):

| Scene | Preset | Particles | ms/frame | body query | self-collision | Strain p99 | Crossings |
|---|---|---|---|---|---|---|---|
| drafted-skirt | Draft | 1,460 | 2.1 | 0.3 | 0.5 | 3.8 % | 0 |
| drafted-skirt | Normal | 3,868 | 8.5 (was 13.0) | 0.7 (was 3.0) | 2.3 | 4.3 % | 0 |
| drafted-tshirt | Draft | 1,910 | 2.6 | 0.4 | 0.5 | 3.3 % | 2 |
| drafted-tshirt | Normal | 5,050 | 10.4 (was 15.4) | 1.1 (was 4.5) | 2.5 | 2.8 % | 17 |
| drafted-tshirt | Fine | 11,321 | 29.7 | 3.1 | 11.5 | 4.0 % | 66 |

Against the Phase 0 baseline on one thread, with seams that weld flat and cloth that keeps off
cloth added since: the drafted skirt 20.1 → 8.5 ms at Normal and 2.1 ms at Draft; the T-shirt
27.7 → 10.4 ms and 2.6 ms. A 2-core laptop at a third of this machine's single-thread speed
should drape the T-shirt at about 60 frames a second on Draft and 30 on Normal.

- Fine's cost is mostly self-collision: smaller cells mean a smaller margin, so the pairs are
  found again more often, and over more particles. Finding them again only for what moved is
  the next saving there.
- The T-shirt's crossings vary from run to run (17 here, 24 before, 0 at 24 × 1): the fold at
  the top of a cap forms, or not, as the seams pull shut.

## After the first look at the app (2026-10-11)

Two things the pictures showed. The fabric beside a seam folded into a sawtooth: hinges across
bias edges were soft everywhere, but that is only right inside a lattice cell, and the band of
irregular triangles along every outline was left nearly free to fold. Only a cell's hinge
shears now. And the seam stood as a ridge: a hinge across a welded seam is a distance link,
which has almost no pull near flat (150° is 3 % off its flat length) while the fabric's
bending is soft. The hinges across welded seams now have their own stiffness,
`Params::seam_compliance`, swept here on the drafted scenes at Normal's 30 × 1 (crease is the
mean angle between the triangles either side of a seam edge; crossings are the cap fold):

| seam compliance | T-shirt crease | T-shirt crossings | skirt crease | demo skirt crease |
|---|---|---|---|---|
| 1.0 (as the fabric) | 29.5° | 21 | 11.9° | 8.9° |
| 0.3 | 24.5° | 66 | 9.1° | |
| 0.1 | 20.0° | 70 | 7.7° | |
| 0.03 | 15.5° | 46 | 6.3° | |
| **0.01 (chosen)** | 14.5° | 39 | 7.5° | 4.5° |
| 0.001 | 13.6° | 41 | 6.4° | |

The skirts' seams then read 4–7°, the T-shirt's 14°, but the user's next picture still showed a
ridge, and `seam_probe` showed why: the hinge links across the seam were up to 10 % *longer*
than their flat rest. A distance link is also a strut along the fabric; on a skirt stretched
over the hips its rest is shorter than the stretched fabric, and the cheapest way to satisfy
it is to buckle the seam into a ridge. So the hinges across welded seams are now held at a
flat angle by a constraint on the angle itself (`solve_hinges`: the triangles' normals over
the opposite vertices' heights, shared back to the edge's ends), with no pull along the
fabric. `Params::seam_compliance` is 100 in that constraint's units (about 80 % of the angle
removed per pass on 12 mm cloth).

| Scene | Preset | ms/frame (14 threads) | Strain p99 | Crossings | Seam crease |
|---|---|---|---|---|---|
| skirt (demo grid) | — | 16.9 | 7.6 % | 0 | 1.0° (was 8.9°) |
| drafted-skirt | Normal | 8.6 | 4.0 % | 0 | 0.3° (was 7.5°) |
| drafted-tshirt | Normal | 10.2 | 2.9 % | 9 | 1.3° (was 14.5°) |

`seam_probe` on the drafted skirt: the hinges across the seam read 0.1° at the median and
2.8° at worst; on the T-shirt 0.2° median, with only the cap fold above a few degrees. The
seam no longer presses the cap fold (9 crossings), and the cost is unchanged.
