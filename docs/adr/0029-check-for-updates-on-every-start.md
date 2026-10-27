# 0029. Check for updates on every start

- Status: accepted
- Date: 2026-10-27
- Amends: 0024

## Context

Handset looked for a newer release at most once a day. Someone who opens it rarely, or who skipped a day
because the first check failed offline, could run an old version for a long time. The check is one small
request with a short timeout on a background thread, so asking once a day saved almost nothing.

## Decision

When automatic checking is on, every start asks GitHub once (5 second limit), in the background, and says nothing
when it fails. The notice is shown at most once per version per run, stays quiet for a skipped version or while
"Remind Me in 3 Days" runs, and stays 10 seconds so it can be clicked. Skipping or postponing is in the right-click menu. The
notice and the update menu are still drawn only in the window, never in captures. A manual check always answers.
The rules of 0024 (no in-place update for package-manager installs, signed releases only) are unchanged.

## Consequences

- One request to api.github.com per start; the setting is now "Check for Updates on Start".
- The `update-checked` setting is no longer written.
