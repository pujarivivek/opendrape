# OpenDrape

Free, open-source 3D garment design for fashion students: draw 2D patterns, sew them
onto a 3D body, and see how the garment drapes. It runs on everyday laptops (4 GB RAM,
built-in graphics) on Windows and macOS, fully offline.

OpenDrape exists because commercial 3D fashion tools cost hundreds of dollars a year
and need gaming-class graphics cards, which shuts out most students in India, Africa
and elsewhere. Digital samples also replace muslin toiles, so less fabric is wasted.

**Status:** early development (milestone M5b: a photo-studio 3D view with studio lighting, soft shadows, soft darkening in folds, fabric sheen and colour-accurate output, on top of M5's five workspaces). Next: dress forms (M3), then the connector for AI tools and Blender.
See `docs/specs/2026-10-09-opendrape-design.md` for the full plan.

## Download

Get the latest build from the [Releases page](https://github.com/pujarivivek/opendrape/releases)
("Nightly build").

- **Mac:** download the `.dmg`, open it, drag OpenDrape to Applications. The first time,
  macOS blocks it because it is not signed with a paid Apple certificate yet: open
  **System Settings → Privacy & Security**, scroll down, click **Open Anyway**.
- **Windows:** run the `-setup.exe` (no administrator rights needed). If Windows says
  "Windows protected your PC", click **More info → Run anyway**. Or download the
  portable `.zip`, unzip it anywhere and run `OpenDrape.exe`.

If the 3D view does not appear on Windows, choose **Help → Graphics → Software (safe mode, slow)**.
If OpenDrape cannot start its graphics at all, it automatically tries safer modes the next
times you open it, and tells you if none of them work.

## Build from source

Install Rust from https://rustup.rs, then:

    cargo run -p opendrape

## License

OpenDrape is free software under the GNU General Public License v3 or later (see `LICENSE`).
Third-party assets and their licenses are listed in `ASSETS.md`.
