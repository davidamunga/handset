# Security policy

## Supported versions

Handset has not had a stable release yet. Fixes are made on `main`.

## Reporting a vulnerability

Please do not open a public issue. Use GitHub's
[private vulnerability reporting](https://github.com/davidamunga/handset/security/advisories/new)
for this repository, and include steps to reproduce.

## Things worth knowing

- Handset controls a phone over adb. Anyone who can run adb against your phone can already do what
  Handset does, so protect your adb keys and only authorise computers you trust.
- The scrcpy server runs on the phone with shell privileges. It is embedded unmodified and pinned to a
  known version (see [ADR 0003](docs/adr/0003-reuse-the-unmodified-scrcpy-server.md)).
- Handset talks to the server through local adb port forwards on `127.0.0.1`.
- Share in a Browser opens a web server on the network the computer is on, on port 8099 when it is free. It
  is plain HTTP: the page and the connection setup travel unencrypted, and only the video, sound and touches
  are encrypted (WebRTC). That is fine at home and not something to rely on over a hostile network.
- Nobody sees or controls anything until the person at the computer allows that one connection. A refused
  address is not asked about again for 30 seconds. Allow and Control lets a viewer do anything a finger,
  keyboard and the Back and Home keys can do on the phone, so only allow people you know. Stopping sharing,
  closing the window or disconnecting the phone ends every viewer.
- Share cannot tell a cafe's Wi-Fi from a home one. It asks before sharing from a computer that is directly
  on the internet, and the approval dialog is the protection everywhere else. Everyone on the same network
  can open the link and trigger that dialog.
- Share sends the phone's H.264 and its sound to the viewer as they are. It cannot lower the bit rate for a
  slow viewer, since the phone's encoder is fixed when mirroring starts. A viewer that cannot keep up loses
  frames and picks up again at the next keyframe.
