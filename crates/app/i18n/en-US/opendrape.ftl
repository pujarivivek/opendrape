app-name = OpenDrape
viewport-no-gpu = The 3D view is unavailable because no graphics adapter could be started.

menu-help = Help
menu-about = About OpenDrape
menu-graphics = Graphics
graphics-auto = Automatic
graphics-dx12 = DirectX 12
graphics-vulkan = Vulkan
graphics-metal = Metal
graphics-gl = OpenGL
graphics-software = Software (safe mode, slow)
graphics-restart-note = OpenDrape restarts to switch graphics mode.

about-tagline = Free 3D garment design for students everywhere.
about-version = Version { $version }
about-graphics = Graphics: { $name } ({ $backend })
about-license = OpenDrape is free software under the GNU GPL, version 3 or later.
about-copy = Copy diagnostics
about-copied = Copied. Paste it into your bug report.

startup-failed = OpenDrape could not start its 3D graphics on this computer. Updating the graphics driver usually fixes this. The next time you open OpenDrape it will try every graphics mode again.

    Details: { $error }

graphics-starting-over = Last time, OpenDrape could not start its 3D graphics in any mode. It will now try again from the start. If this keeps happening, please update your graphics driver.

toolbar-play = Play
toolbar-pause = Pause
toolbar-reset = Reset
garment-skirt = A-line skirt
garment-bodice-proxy = Fitted tube (collision test)
overlay-stats = { $fps } fps · simulation { $ms } ms per step · { $points } points
overlay-stats-paused = simulation { $ms } ms per step · { $points } points
