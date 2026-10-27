---
"handset": patch
---

Linux: `handset --doctor` now lists missing helper programs (`xdg-open`, a clipboard tool, `zenity` or `kdialog`) and explains a phone that is blocked by missing udev rules. The window has a class and icon so desktops match it to its entry, and `scripts/package-linux.sh` builds a tarball (and an AppImage when `appimagetool` is installed).
