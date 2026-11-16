---
"handset": minor
---

`handset ctl` now waits for the phone to show what was done, so a script or an agent can send commands one after another without sleeps and `ui` right after a tap describes the new screen, not the old one. New `wait-text` and `wait-gone` wait for something slow to appear or go, and a missing item on a scrolling screen says it may be out of view.
