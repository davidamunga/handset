# 0047. Wait for the phone to show what was done

- Status: accepted
- Date: 2026-11-16

## Context

An agent was given a plain task through `handset ctl` (read the battery level and the screen timeout in
Settings, and check that search finds a Wi-Fi setting), using only `handset ctl help`, with no sleeps. It
finished the task, but along the way:

- `ui` straight after `open` or `tap-text` described the screen as it had been, because an action returns when
  it has been sent and Android takes about a second to show it. The basic loop, act and then look, gave a stale
  answer with no sign that it was stale.
- A `scroll` sent straight after `key back` was lost: the page was still turning.
- Search results appeared several seconds late, so there was no way to wait for them except guessing a sleep.
- Text that was further down a scrolling list was reported as not on the screen, with no hint to scroll.

## Decision

We will make the commands wait for the phone to catch up, and add a way to wait on purpose.

- **Looking waits for what was done.** `ui`, `tap-text` and `tap-id` after an action wait until at least 0.4 s
  have passed and the picture has held still for 0.3 s, and read the screen after 2.5 s at the latest, so an
  animation that never stops (a game) does not hold them up for long. With no action in the last few seconds
  there is no wait.
- **Acting waits too.** The same wait comes before the next touch, key, text or app launch, so a tap is not sent
  into a page that is turning.
- **`wait-text <text> [<n>s]` and `wait-gone <text> [<n>s]`.** Look again and again (the helper makes that cheap)
  until the words appear or go, or up to 30 s. A look that fails is tried again; if it keeps failing, the reason
  is the answer.
- **Say when it may be out of view.** If a text or id is not found and something on the screen scrolls, the
  error says so.

## Alternatives considered

- **Leave the waiting to the agent.** Every agent would write its own sleeps, too long to be quick and too
  short to be right, and the stale answer would still look like a real one.
- **Wait for a fixed time after every action.** Slow when nothing changes, and wrong when something takes
  longer. Watching the picture finds the real moment.
- **Wait for accessibility events from the phone.** More exact, but needs the helper to stay connected and
  forward events. A later step if watching the picture is not enough.

## Consequences

- An act-then-look loop gives the right screen without sleeps. A command that follows another costs about
  0.7 s, which is small next to the time an agent spends thinking and a risk-free price for a correct answer.
  Four `key del` in a row take about three seconds.
- Watching the picture can be fooled: a change that does not show (a hidden state), or one that starts after
  more than 0.4 s of nothing, is missed. `wait-text` is the answer to that.
- `open` and `type` can still do nothing without saying so (a name that fits two apps, no text field in
  focus). The package on the first line of `ui` and the text of the field show what happened; the guide says
  to look.
- Not yet: a command that lists the phone's apps.
