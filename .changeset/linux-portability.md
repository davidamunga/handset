---
"handset": patch
---

Linux: the tarball is now built on Ubuntu 22.04, so it runs on Ubuntu 22.04 and newer and Debian 12 and newer (the packaging script refuses a build that needs a newer glibc). GTK is not needed to build or run. Alpine builds from source with a dynamic musl link, and there is a Nix package for NixOS.
