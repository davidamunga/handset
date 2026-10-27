---
"handset": patch
---

Fix memory that grew by gigabytes, until the app stopped responding, while the Handset window was covered by other windows during mirroring. Each new phone frame was copied to the GPU but never released while the window could not be drawn. Handset now stops drawing while its window is hidden and starts again when it shows.
