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

tool-edit = Edit
tool-pen = Pen
tool-rectangle = Rectangle
tool-add-point = Add point
tool-edit-tip = Select and move points, curve handles, edges and whole pieces.
tool-pen-tip = Draw a piece point by point. Drag while placing a point to make a curve.
tool-rectangle-tip = Draw a rectangular piece.
tool-add-point-tip = Add a point on an edge.
units-cm = cm
units-inch = inch
toolbar-show-lengths = Show lengths
toolbar-fit = Fit (F)
toolbar-fit-tip = Show all pieces.
box-length = Length
box-angle = Angle
box-width = Width
box-height = Height
piece-default-name = Piece
notice-need-three-points = A piece needs at least 3 points.
notice-bad-number = Please type a valid number. Lengths must be at least 0.1 mm and at most 10 m.
notice-too-close = Too close to an existing point.
notice-name-empty = A piece needs a name.
notice-min-points = A piece needs at least 3 points, so this point can't be deleted.

panel-title = Properties
panel-hint = Select a piece, a point or an edge to see and change its measurements.
panel-piece = Piece
panel-name = Name
panel-grain = Grain angle
panel-area = Area
panel-perimeter = Perimeter
panel-delete-piece = Delete piece
panel-edge = Edge
panel-length = Length
panel-keep-fixed = When the length changes, keep fixed:
panel-anchor-start = Start point
panel-anchor-end = End point
panel-curved = Curved
panel-point = Point
panel-x = X
panel-y = Y
panel-smooth = Smooth curve point
panel-delete-point = Delete point
hint-edit = Click to select. Drag points, curve handles, edges or whole pieces. Delete removes the selection.
hint-pen-start = Click to place corners, or press and drag to make a curve point.
hint-pen-drawing = Type a number for an exact length. Click the first point or press Enter to finish; Backspace removes the last point; Esc cancels.
hint-rectangle = Drag to draw a rectangle, or click and type its width and height.
hint-add-point = Click on an edge to add a point there.
status-cursor = x { $x }   y { $y }

menu-file = File
menu-new = New
menu-open = Open…
menu-save = Save
menu-save-as = Save As…
menu-quit = Quit OpenDrape
menu-edit = Edit
menu-undo = Undo
menu-redo = Redo
file-type-project = OpenDrape project
file-untitled = Untitled
window-title = { $name } — OpenDrape
window-title-unsaved = • { $name } — OpenDrape
unsaved-title = Save your changes?
unsaved-body = “{ $name }” has changes that are not saved yet. If you don't save, they will be lost.
unsaved-save = Save
unsaved-discard = Don't save
unsaved-cancel = Cancel
error-title = Something went wrong
error-open = This file could not be opened: { $error }.
error-save = The project could not be saved: { $error }.
error-ok = OK
