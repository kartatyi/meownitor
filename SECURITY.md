# Security policy

## Reporting a vulnerability

Please do not open a public issue. Report it privately through
[GitHub's security advisories](https://github.com/kartatyi/meownitor/security/advisories/new). You
will get an answer within a week. Once a fix is released, the advisory is published with credit to
you, unless you prefer otherwise.

Only the latest release gets security fixes.

## What Meownitor touches

Worth knowing when you judge what is a vulnerability:

- **Claude Code's sign-in.** To show the plan limits, the widget reads Claude Code's OAuth token from
  `~/.claude/.credentials.json` and sends it only to Anthropic: `api.anthropic.com` for the usage
  numbers and `platform.claude.com` to refresh the token when it is about to expire, writing the new
  one back as Claude Code does. The token is never logged and never sent anywhere else.
- **Claude Code's settings.** `~/.claude/settings.json` changes only when you install or remove the
  hook (from the settings or the installer) or when an update moves the hook's copy, and only the
  widget's own entries change; the file is backed up first.
- **The hook.** `meownitor-hook` runs on Claude Code's session events. It reads the event from
  stdin and writes the session's state, the current tool's name and one short detail (a file name, a
  command's description, a search pattern) into `~/.meownitor`. It makes no network requests.
- **Question rounds.** Pages a Claude Code session writes are served to a window of the widget's own
  through a custom `round` scheme, from the widget's rounds folder only (HTML, CSS, JavaScript and
  images).

Nothing else leaves your machine: there is no telemetry.
