---
"handset": patch
---

Handset now builds on Linux (Ubuntu 24.04 is checked in CI). Links and captures open with `xdg-open`, "copy capture" and the phone clipboard sync use `wl-copy`/`xclip`/`xsel`, and the address prompt uses `zenity` or `kdialog`. It is experimental: it has not been run on a real Linux desktop yet, and there is no release file.
