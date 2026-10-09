# OpenDrape M2b: what to try

This build finishes the pattern tools: seam allowance, notches, internal lines, pieces cut
on the fold, and mirrored left/right pairs. Pieces still don't go onto the body; sewing comes
in M3.

## Check these

- [ ] Every piece shows a light blue band around it: the 1 cm seam allowance. The outer line
      is where you cut. **Show seam allowance** at the top hides and shows it.
- [ ] Click a piece. In **Properties**, change **Seam allowance** to **1.5**: the band
      widens all round.
- [ ] Click one edge. Type **2** in its **Seam allowance**: only that edge changes. **Same as
      piece** puts it back. Tick **Hem**: that edge gets 3 cm, and the corners at its ends
      fold in so the hem can turn up.
- [ ] Switch units to inches and type an allowance over the limit: the message names the
      limit in inches.
- [ ] Draw half a skirt front. Click its straight centre edge, then **Set as fold line**: the
      whole piece appears, the other half pale, with a dashed fold line and "Place on fold".
      Drag a point on the drawn half: the pale half follows. There is no allowance along the
      fold.
- [ ] Press **N** (Notch) and click an edge: a small notch appears in the allowance. Point
      near an edge and type **5**, then **Return** (Enter): a notch exactly 5 cm from the
      nearer end. Click a notch with **Edit (Z)**, then try **Double** and **V** in
      Properties.
- [ ] Untick **Show seam allowance**: the notches move onto the edge of the piece (5 mm
      deep, pointing in) and can still be clicked there. Tick it again: they go back out.
- [ ] Press **Delete** right after placing a notch: it disappears, without switching tools.
- [ ] A notch placed exactly on a corner doesn't stop you grabbing and moving that corner.
- [ ] Press **L** (Internal line). Click inside a piece twice and press **Return**: a dashed
      line. Click three points and then the first again: a closed shape. Choose **Cut-out**
      for it in Properties. Lines can't leave their piece.
- [ ] Draw a curved line (press and drag while drawing). Select it with **Edit (Z)** and drag
      its handles: the line bends, and it still can't leave the piece.
- [ ] On a folded piece, start a line right on the fold line: it is accepted, whichever side
      of the fold your first click lands on.
- [ ] Press **Delete** with a finished line selected in the **Internal line** tool: the line
      is removed.
- [ ] Select a back panel and click **Make mirrored pair**: its mirror image appears beside
      it, named "... (mirror)". Drag a point on either one: the other changes too. Drag the
      middle of one: only that one moves. **Break pair** makes them separate pieces.
- [ ] While typing a length and angle with the pen, **Tab** moves between the two boxes and
      back, and never closes them.
- [ ] Edge lengths sit just outside each piece, not on the line.
- [ ] **Draft a skirt front on the fold** with a 1 cm allowance and a 3 cm hem, put notches at
      the hip, and add a mirrored pair of back panels. **File → Save As…**, quit, reopen and
      **File → Open…**: everything is back. Undo several steps with **Cmd+Z**.
- [ ] Your M2a files still open; their pieces now show a 1 cm allowance.
- [ ] Change something, then quit from the Dock (right-click the OpenDrape icon → **Quit**).
      Open OpenDrape again: it offers to **Restore** your unsaved work.

## Known limits in this build

- Cmd+H (hide OpenDrape) does not work in this build.
- Internal lines aren't checked again when you reshape their piece, so move them back inside
  yourself if needed.
- Very large pieces with thousands of curved edges can take a moment to show their seam
  allowance.

If anything looks wrong, take a screenshot, then choose Help → About OpenDrape and
click **Copy diagnostics**.
