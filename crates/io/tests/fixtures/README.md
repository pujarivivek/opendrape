# Frozen project files

One folder per project format version. The files in them are kept forever and are never
regenerated: `fixtures.rs` checks that every future build still opens each of them and sees
exactly the project described there. When the format changes (`SCHEMA_VERSION` goes up), add a
new folder for that version with a hand-written file and a test, and leave the old ones alone.

Folders: v1 (M2a), v2 (M2b: seam allowance, notches, internal lines, fold, twin), v3 (M4a:
seams, 3D placements), v4 (M4b: seam sides between any two points of an outline, pins),
v5 (M5c: the dress form).
