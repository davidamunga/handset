---
"handset": minor
---

`handset ctl ui` lists what is on the phone's screen as text, and `tap-text` and `tap-id` tap an element by what it says or its id, so a script or an agent no longer has to guess where a button is. Android only. See `docs/agents.md`. The screen is described once and kept while it does not change, and read ahead after each action, so repeated calls are almost instant.
