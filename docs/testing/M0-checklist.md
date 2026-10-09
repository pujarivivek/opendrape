# OpenDrape M0: what to try

This first build only checks the foundations: the app installs, opens, and draws 3D on
your computer. There is no garment yet, only a test cube.

## Install on a Mac

1. Go to the project's **Releases** page and open **Nightly build**.
2. Download the file ending in `macos-universal.dmg` and double-click it.
3. Drag **OpenDrape** onto the **Applications** folder in the window that opens.
4. Open your Applications folder and double-click **OpenDrape**. macOS says it cannot
   check the developer. Click **Done**. (The app is not yet signed with a paid Apple
   certificate.)
5. Open **System Settings → Privacy & Security**, scroll down to the message about
   OpenDrape, click **Open Anyway**, enter your password, then click **Open**.
   You only do this once.

## Check these

- [ ] A window called **OpenDrape** opens with an orange-brown cube in a light grey area.
- [ ] Dragging on the cube turns it. Scrolling zooms in and out.
- [ ] The cube's top looks lighter than its sides (it is lit from above).
- [ ] **Help → About OpenDrape** shows your graphics, e.g. "Graphics: Apple M4 Max (Metal)".
- [ ] Click **Copy diagnostics**, then paste into Notes: you see the version, OS and graphics lines.
- [ ] **Help → Graphics** shows **Automatic** selected.
- [ ] Make the window very small, then very large: nothing breaks.
- [ ] Quit (Cmd+Q) and open it again: it opens normally.

If anything fails, write down what you did and what happened, and paste the copied
diagnostics with it.

## For Windows testers

Download `…-windows-x64-setup.exe` and run it. If Windows says "Windows protected your
PC", click **More info → Run anyway**. It installs without asking for an administrator.
Or download the `portable.zip`, unzip it anywhere, and run `OpenDrape.exe`.

Also check:
- [ ] **Help → Graphics → Software (safe mode, slow)**: OpenDrape restarts and still shows the cube.
- [ ] **Help → Graphics → Automatic**: it restarts back to normal.
