# Changelog

## Unreleased

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
