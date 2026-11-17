# Changelog

## 0.3.0

### Sessions

- A session that waits for you — a permission request, a question — stays under «Waiting for you»
  until it is answered or stopped. After half an hour of silence it used to drop into the idle
  group, and the pet stopped asking. A question round stays for as long as it has no answer.
- A turn that ended on an API error shows as «Stopped with an error», with the reason: limit
  reached, server error and the like. For that the hook listens to a twelfth event, `StopFailure`.
- A session waiting for permission shows what it asks to allow, in full — the command itself, the
  whole path; one whose turn is over shows the beginning of what Claude said.
- Permissions on the card, off until turned on in the settings: a permission request comes to the
  session's row with **Allow**, **Deny** and **In the terminal**. The hook waits up to two minutes
  for the click and otherwise leaves the request to Claude Code's own dialog. An answer can only
  come from the widget's own window, and `widget.log` keeps each one.
- A click on a terminal session unfolds two actions: copy its `claude --resume` command, open its
  folder. Names and details cut short show the whole of them on hover.
- A first start shows the card by itself, and without the Claude Code hook the card says so and
  has the **Install** button.

### Sounds and notifications

- A sound of your choice for each: a session starts waiting for you, its turn is over, it stops
  with an error. A dozen to pick from — three meows, two quacks, a croak, a cluck and a plop are
  recordings (CC0, from Freesound; see `app/src/sounds/README.md`), a cuckoo, a pew, a coin and
  the old chirp are made on the spot. A volume slider, a switch for each.
- On request the sound comes again every five minutes while a session keeps waiting, and a system
  notification goes with it. **Do not disturb for an hour** in the tray menu silences it all.

### The window

- The widget also docks at the top edge of the screen, to one of three magnets — the left corner,
  the centre or the right corner — and the card drops down from there on hover.
- Three looks at an edge, picked in the settings: the strip of dots (as before), the pet peeking out
  (half of it from a side, upside down from the top) or a thin line that pulses and lets the pet peek
  out when a session waits for you. Collapsed at an edge, the widget nudges out once when a session
  starts waiting.
- Motion: at an edge the card slides out of the screen edge on hover and back into it; the card
  unfolds from the pet and folds back into it, and turns over to the settings in one movement.
- The pet comes back to where you put it after its card is opened and folded again.
- A card taller than the screen has room for scrolls inside, the header and the limits staying put.
- The tray menu opens the card and the settings, hides the widget for an hour and opens the folder
  with data and logs; on Windows a click on the icon opens the card and hovering says who waits
  and who works.
- Windows: the widget hides while a full-screen window is in front on its monitor — a film, a
  slide show, a game — unless turned off in the settings.
- Windows: the widget no longer gets lost behind other windows some time into a run, and the
  content no longer jumps at the right edge when the window changes size.
- One widget at a time: started while one is running, a second one leaves.
- The unused part of a limit bar has a lighter track, so a nearly spent limit no longer looks full.

### Question rounds

- The `meownitor-round` skill comes with the widget: installing the hook puts it into
  `~/.claude/skills/`, every start of an updated widget brings the new one, removing the hook takes
  it away. The page kit is built into the widget and served next to every page.
- `meownitor-hook ask <name>` checks the page before waiting — no title, no question for the kit,
  no way for the answer back — and reports it at once. Given a path to a page made elsewhere, it
  copies the page in first. `wait` still works.
- A round keeps the draft of an answer: a window closed by accident comes back as it was left.
- An answer with no pick but a comment reads `1 — own option`. The skill is split into a short
  `SKILL.md` and `reference.md` for the rest of the kit.

### Housekeeping

- The data folder is cleaned up: a session's state file goes a week after its last event, a
  session's rounds a month after the last change in them — never while one still waits for an
  answer. The round folders are no longer read in full three times a second.
- The first start of 0.3.0 brings an installed hook up to date — the `StopFailure` event, a longer
  wait on `PermissionRequest` — backing up `~/.claude/settings.json` first, as always.
- `events.log` lines no longer run into each other when two hooks write at once.

## 0.2.0

- macOS and Linux. macOS gets one universal disk image for Apple silicon and Intel. It is not
  signed with a Developer ID, so the first start needs **Open Anyway**. macOS support is
  experimental: it has run on a Mac built from source (macOS 26.3, Apple silicon), the disk image
  only in CI so far. Linux gets `.deb`, `.rpm` and an AppImage for x86-64; under Wayland the widget
  runs through XWayland.
- macOS: the plan limits, from Claude Code's sign-in in the Keychain — read only, never refreshed. An
  expired one keeps the last numbers on the card until Claude Code renews it.
- Petting follows the polled cursor rather than mouse moves: macOS sends none to a window that is
  never active.
- The data folder is `~/.meownitor` on every system, and the `meownitor-round` skill calls the hook
  by one path everywhere.
- The autostart switch is worded for each system: start with Windows, open at login, start on login.

## 0.1.0

The first public release. Windows 10 and 11.

- A pixel pet on the desktop — a cat, a blob or a ghost — that follows the cursor with its eyes, likes
  being petted, sleeps when nothing happens and gets tired near a plan limit. Three ways to keep it:
  on its own, sitting on the card, or as a thin strip at a screen edge that slides the card out on
  hover.
- Claude Code sessions at a glance, from the CLI and Claude Desktop alike: who is waiting for you,
  who is working and on what, whose turn it is; a click opens the session in Desktop.
- Plan limits — the 5-hour and the weekly one, with their reset times.
- Questions with pictures: with the `meownitor-round` skill a session draws its options on one page,
  the widget chirps, you pick in a window and the answer goes straight back to the session —
  whenever you get to it: an answer sent after the session stopped waiting comes with your next
  message.
- Settings on the back of the card: the character, English or Ukrainian, the sound, starting with
  Windows and the Claude Code hook — installed or removed with a click, backing up
  `~/.claude/settings.json` first.
- Gets itself unstuck: should its WebView2 freeze (seen when Windows ran out of memory), the widget
  takes the mouse back within seconds, then resumes WebView2 or starts over; `widget.log` says when.
- One data folder, `~/.meownitor`, for sessions in Claude Desktop and in a terminal alike
  (Desktop gives everything it starts its own copy of AppData).
- An installer for the current user (no admin rights) and a portable zip. Uninstalling takes the
  hook out of Claude Code's settings; installing again puts it back.
