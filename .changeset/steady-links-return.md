---
"handset": patch
---

When the connection to the phone drops, Handset now shows "Connection lost" on the connect screen and starts mirroring again by itself when the phone is back, instead of quitting. Phones on Wi-Fi are reconnected with `adb connect` every few seconds.
