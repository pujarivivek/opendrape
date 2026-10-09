# OpenDrape M2a: what to try

This build adds the pattern window. The 3D body stays on the left; on the right
is a grid, the pattern table, where you draw pattern pieces. Pieces don't go onto
the body yet: sewing them on comes in M3.

## Check these

- [ ] OpenDrape opens with the 3D view on the left and the pattern table on the
      right. Drag the divider between them: both sides resize.
- [ ] Press **H** (or click **Pen (H)**). Click four corners, then click the first
      point again: the piece fills in and is called "Piece 1".
- [ ] With the pen, click a point, then type **50**: a small box with **Length** and
      **Angle** opens with your number in it. Press **Return** (Enter): the next
      point is exactly 50 cm away, towards your mouse. To set the direction too, type
      a number in **Length**, press **Tab**, type **90** in **Angle**, and press
      **Return**: that edge goes straight up.
- [ ] While drawing, press and drag instead of clicking: that point becomes a
      smooth curve.
- [ ] **Cmd+Z** while drawing removes only the last point. **Esc** cancels the
      piece.
- [ ] Click **Rectangle (S)**, or press **S**, and drag: you get a rectangle. Or
      click once and type its **Width**, press **Tab**, type its **Height**, and
      press **Return**.
- [ ] Click **Edit (Z)**, or press **Z**. Drag a corner, an edge, and the middle of
      a piece: each moves. Select a curved piece and drag one of its blue handles
      to change the curve.
- [ ] Click an edge. In the **Properties** panel, type **45** in **Length** and
      press **Return**. Under **When the length changes, keep fixed:**, choose
      **Start point** or **End point** to say which end stays put.
- [ ] Click **Add point (X)**, or press **X**, then click an edge: a new point
      appears there. Select a point and press **Delete** (the Backspace key on a
      Mac keyboard) to remove it.
- [ ] Switch between **cm** and **inch** at the top of the pattern window: the
      numbers change, the pattern doesn't.
- [ ] **Draft a skirt panel from scratch.** For example: a 45 cm waist, 60 cm side
      seams and a curved hem. Then **File → Save As…**, choose **File → Quit
      OpenDrape**, reopen it and use **File → Open…**: everything is back.
- [ ] Undo 10 steps with **Cmd+Z** (Edit → **Undo**), then redo them with
      **Shift+Cmd+Z** (Edit → **Redo**).
- [ ] Change something, then close the window, press **Cmd+Q**, or choose
      **File → Quit OpenDrape**: OpenDrape asks whether to save. **Cancel** keeps
      you working.
- [ ] The 3D skirt still drapes as it did in M1.

## Known limits in this build

- Quitting from the Dock (right-click the OpenDrape icon → Quit) does not ask
  about unsaved changes yet. Use Cmd+Q or File → Quit OpenDrape instead.
- Cmd+H (hide OpenDrape) does not work in this build.

If anything looks wrong, take a screenshot, then choose Help → About OpenDrape and
click **Copy diagnostics**.
