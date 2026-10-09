# OpenDrape M1: what to try

This build shows a real 3D body and simulates fabric falling onto it. There is no pattern
drawing yet: the two garments are built in.

## Check these

- [ ] OpenDrape opens showing a woman's body in a light grey room.
- [ ] A blue skirt appears around the hips, its side seams pull together, and it falls
      into place at the waist within about 5 seconds, then stops moving.
- [ ] Turn the view by dragging and zoom by scrolling: the skirt never passes through the body
      (no skin showing through the fabric).
- [ ] While the fabric is moving, the text at the top left shows a speed of at least 20 fps.
- [ ] Once the skirt has settled, the simulation pauses itself (the button changes to **Play**),
      so OpenDrape doesn't keep your computer busy.
- [ ] **Pause** freezes the fabric; **Play** continues it.
- [ ] **Reset** starts the drape again from the beginning.
- [ ] Click **Fitted tube (collision test)**: a tight tube hugs the chest and waist with no
      skin poking through. Click **A-line skirt** to go back.
- [ ] Quit and reopen: it starts normally.

If anything looks wrong, take a screenshot and copy Help → About → Copy diagnostics.
