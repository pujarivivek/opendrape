# OpenDrape M5b: what to try

This build gives the 3D view a photo-studio look: a soft grey studio, neutral studio lighting,
soft shadows, soft darkening in folds (ambient occlusion), fabric that has a sheen instead of
looking like plastic, and colours you can trust. While you turn the view or the cloth drapes, a
lighter version keeps it smooth; once everything stops, the full look builds up in about a
second and the view goes quiet.

## Check these

- [ ] **The studio.** The 3D view shows a light-grey floor that fades into a soft grey
      backdrop. Turn the view all the way round: there is no edge where floor meets backdrop.
- [ ] **The form** is a matte linen beige.
- [ ] **Floor shadow.** Under the form there is a soft shadow on the floor, darkest where the
      form meets it, fading out over a few centimetres. No hard edge.
- [ ] **Drape the T-shirt** (M4b checklist). Once it settles:
      - the folds have soft darkening in them, and so do the places where the sleeve meets the
        body and the garment meets the form;
      - the key light (from the front-right, above) casts soft shadows across the cloth;
      - the fabric glows a little at its edges as it turns away from you (sheen), with no
        shiny highlight;
      - the inside of the garment, where you can see it, is a little darker, like a lining.
- [ ] **Sharp when still.** Stop turning the view: within about a second the edges of the
      garment get smoother and the shadows get cleaner. Your laptop's fan should quieten
      once it's done (the view stops redrawing).
- [ ] **Smooth while moving.** Turn the view and let the cloth drape: it stays smooth.
- [ ] **Colours you can trust.** Pieces are still one fixed fabric blue (colour choice comes
      with Texturing). Place a piece at the front, view from the front (the first little
      form in the 3D view's corner), wait a second, and open **Digital Color Meter**
      (Applications → Utilities) set to "Display in sRGB". Point at the middle of the piece:
      it reads close to **115, 162, 218** (within about 5 on each).
- [ ] **View → 3D quality.** It lists Auto (with the level it picked, e.g. "Auto (High)"),
      Basic, Medium and High. Try each: Basic is plainest while moving and still gets
      shadows and soft darkening once still; High is the fullest. Quit and reopen: your
      choice is remembered.
- [ ] **View → Lighting.** Soft, Balanced and Sculpted (the default). Sculpted shows the
      shape of the form and the folds most strongly; Soft is the even softbox look. The
      fabric facing you stays the same colour in all three. Your choice is remembered.
- [ ] **Floor grid and shadow.** Faint lines every 10 cm, stronger every metre, fading into
      the backdrop; the form's shadow falls behind-left, about as long as the form is tall,
      with no cut-off end.
- [ ] **On a weak laptop** (if you can try one): Auto picks Basic or Medium, and turning the
      view stays smooth.

If something looks wrong, use **Help → About OpenDrape → Copy diagnostics** and paste it
into your message with a screenshot.
