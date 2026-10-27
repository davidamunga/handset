---
"handset": patch
---

QR pairing no longer waits forever when adb cannot see the phone on the network. Handset now looks for phones itself and merges that with adb's list. The QR screen has a "Pair with a code" button, and after 12 seconds without a phone it says what usually gets in the way (Wireless debugging screen, same Wi-Fi, no VPN).
