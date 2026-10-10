# OpenDrape M4b: what to try

This build sews part of an edge, puts sleeves on the arms, and lets you pull and pin the
fabric while it drapes. You draft a T-shirt, sew its sleeve caps into the armholes, place the
pieces, press **Play**, and change the pattern while it drapes.

## Check these

- [ ] **Fit moved.** Press **Cmd+0** (Ctrl+0 on Windows): every piece fits the window. The
      toolbar's **Fit** button shows the new shortcut. **F** is now the Free Sew tool.
- [ ] **Draft a front on the fold.** With **Pen (H)**, draw half a T-shirt front about 25 cm
      wide and 60 cm tall: hem, side seam, an armhole curving in to the shoulder, a sloping
      shoulder, and a neckline down to the centre front. Click the centre-front edge, then
      **Set as fold line**.
- [ ] **Draft a back the same way**, to the right of the front, with a shallower neckline,
      and set its centre back as the fold line.
- [ ] **Draft a sleeve.** With **Rectangle (S)**, draw a piece 30 cm wide and 13 cm tall.
      With **Edit (Z)**, click its top edge, tick **Curved**, and drag both handles up about
      13 cm: that is the cap. With **Notch (N)**, click the top of the cap. Select the sleeve
      and click **Make mirrored pair**.
- [ ] **Sew the straight seams with W:** the front's shoulder to the back's shoulder, the
      front's side to the back's side, and the sleeve's left edge to its right edge (the
      underarm).
- [ ] **Sew the cap into the front armhole with F.** Press **F** (Free Sew). Click the cap's
      left corner (where it meets the underarm edge), then the notch: a thick line runs along
      half the cap. Click the bottom of the front's armhole, then the front's shoulder point.
      A coloured seam line appears along half the cap and the whole armhole. The mirrored
      seams (the front's pale half, the other sleeve) appear by themselves.
- [ ] **Sew the other half of the cap into the back armhole.** Click the notch, then the
      cap's right corner; then the back's shoulder point, then the bottom of the back's
      armhole.
- [ ] Still in Free Sew, click one corner twice: "That side is too short to sew: pick
      points more than 1 mm apart." Press **Esc**.
- [ ] Click two points on an edge, holding **Shift** on the second: the side goes the long
      way round the piece. Press **Esc**.
- [ ] Click two points along the front armhole, which is sewn already: "Part of this is
      already sewn." Press **Esc**.
- [ ] Click a cap seam's coloured line: **Properties** shows both lengths.
- [ ] **Notches that don't match.** Draw two rectangles. With **Notch (N)**, add a notch to
      the middle of the first one's bottom edge. With **W**, sew that edge to the second
      one's bottom edge, then click the seam's line: the panel says "Notches don't match: 1
      on one side, 0 on the other." Delete the two rectangles.
- [ ] **Place the pieces.** In the 3D view, select the front. In **Properties**, under
      **3D placement**, type **105** in **Position Y**, and the same for the back. Right-click
      the front, then **Place at front**; the back, then **Place at back**. Right-click the
      sleeve, then **Place at left arm**: it wraps round the left arm, and its mirror wraps
      round the right arm.
- [ ] Press **Play**. The T-shirt settles on the body with the sleeves on the arms. Under
      "Press Reset to move pieces." it says "Drag the fabric to pull it. Right-click it to pin
      it there; drag a pin to move it."
- [ ] **Change the pattern while it drapes.** In the pattern window, with **Edit (Z)**, drag
      the sleeve's two hem corners about 5 cm down. The drape carries on with longer sleeves,
      without going back to arranging. **Cmd+Z**: the sleeves are short again, still draping.
- [ ] **Pull the fabric.** In 3D, press on the front hem and drag: the fabric follows the
      pointer. Let go: it falls back.
- [ ] **Pin it.** Right-click the front hem, then **Pin here**. A red ring appears there in
      3D and on the front in the pattern window. Drag the ring in 3D: the fabric follows it.
      **Cmd+Z** puts the pin back where it was. Right-click the ring, then **Remove pin**.
- [ ] While it drapes, the 3D placement fields are greyed out under "Placements apply after
      Reset.", and so is **Place at…**.
- [ ] Press **Reset**: the pieces are back where you placed them.
- [ ] **File → Save As…**, quit, reopen and **File → Open…**: the free seams and the pins are
      back. Your M4a files still open, with their seams as they were.

## Known limits in this build

- Cmd+H (hide OpenDrape) does not work in this build.
- Placements don't change while the garment drapes: press **Reset** first.
- A grab pulls one point at a time, and a pin holds its spot exactly.
- Every piece is one light cotton until fabrics arrive.
- The garment drapes on the bundled body; dress forms come in M3.

If anything looks wrong, take a screenshot, then choose Help → About OpenDrape and
click **Copy diagnostics**.
