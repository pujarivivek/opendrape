# OpenDrape M4a: what to try

This build sews pattern pieces together and drapes them on the body. You draw a skirt, sew
it, place the pieces round the body in 3D, and press **Play**. Sleeves, pinning fabric in 3D
and the T-shirt come in M4b.

## Check these

- [ ] **Draft a skirt front on the fold.** With **Rectangle (S)**, draw a piece about 30 cm
      wide and 55 cm tall. With **Edit (Z)**, drag its top-right corner about 12 cm to the
      left, so the waist is narrower than the hem. Click its left edge, then **Set as fold
      line**.
- [ ] **Draft a back.** Draw another rectangle, 30 × 55 cm, to the right of the front. Drag
      its top-left corner about 12 cm to the right. Select it and click **Make mirrored
      pair**.
- [ ] **Sew the side seam.** Press **W** (Sew). Click the front's slanted right edge near the
      hem, then the back's slanted left edge near the hem. A coloured line with a number
      appears just inside both edges. The same seam appears by itself between the front's
      pale half and the mirrored back.
- [ ] **Sew the centre back.** Still in Sew, click the back's straight right edge near the
      hem, then the mirror's straight left edge near the hem. Without it, the skirt is open at
      the back and slides off the body.
- [ ] Click a sewn edge again in Sew: "This edge is already sewn."
- [ ] In Sew, click the fold line down the middle of the front: "The fold line is inside the
      piece and can't be sewn."
- [ ] Click one edge, then press **Cmd+Z** before the second: the half-made seam is cancelled
      and the seams you made earlier stay.
- [ ] Click a seam's coloured line. **Properties** shows each side's length. Click **Flip**:
      the thin lines joining the seam's ends now cross. Click **Flip** again.
- [ ] In the 3D view, the pieces stand in front of the body. Right-click the front, then
      **Place at front**: it curves round the body. Right-click the back, then **Place at
      back**: the back and its mirror meet at the centre back.
- [ ] Click the front in 3D: it turns orange, and it is selected in the pattern window too.
      Drag the green arrow down until the top of the skirt is at the waist; the label says
      how far ("27.0 cm down"). Drag a ring to turn the piece; hold **Shift** for 15° steps.
      **Cmd+Z** undoes a whole drag; press it again to step back through your gizmo moves, one
      drag at a time. Clicking an arrow or a ring leaves the piece selected. Press **Esc** in
      the middle of a drag: the piece jumps back and the drag is not an undo step.
- [ ] Do the same for the back (its mirror follows it).
- [ ] Press **Play**. The arrows and rings go at once. The seams pull shut and the skirt
      settles on the body, with nothing poking through. "Press Reset to move pieces." shows
      while it drapes. **Front**,
      **Back**, **Left side** and **Right side** turn the view.
- [ ] Press **Reset**: the pieces are back where you placed them.
- [ ] In **Properties**, under **3D placement**, type **75** in **Position Y**: the piece
      moves to 75 cm up.
- [ ] **File → Save As…**, quit, reopen and **File → Open…**: the seams and the arrangement
      are back. (A reopened file starts with a fresh undo history, so **Cmd+Z** has nothing to
      undo until you change something.)
- [ ] Your M2b files still open, with no seams yet.
- [ ] Draw a piece with the pen whose outline crosses itself (a figure of eight), and press
      **Play**: a note names it ("… couldn't be made into fabric: its outline crosses
      itself.") and the rest still drapes.

## Known limits in this build

- Cmd+H (hide OpenDrape) does not work in this build.
- Seams join whole edges. Sewing part of an edge (a sleeve cap) comes in M4b.
- Changing the pattern while it drapes returns to arranging. Live updates come in M4b.
- Pieces you never place hang in front of the body, and fall to the floor when you press
  **Play**.
- A cut-out drawn as a half on the fold line is left out of the fabric: the piece is made
  whole there, and a note says so when you press **Play**.

If anything looks wrong, take a screenshot, then choose Help → About OpenDrape and
click **Copy diagnostics**.
