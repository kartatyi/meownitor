---
name: meownitor-round
description: Ask the user through Meownitor, the desktop widget — one HTML page with the options drawn side by side, answered in a window on their desktop; the answer comes back on its own. Use when a choice is easier made by seeing it (UI looks, layouts, designs, diagrams), to show a result for the user's OK with what changed framed and numbered, or when the user asks to be asked visually.
---

# Ask through the widget

Meownitor (the pixel cat on the user's desktop) shows your session as asking and opens your page in
a window when the user presses «Answer». Their answer comes back through a background waiter.

## Steps

1. **The round folder** (Bash): `"${USERPROFILE:-$HOME}/.meownitor/bin/meownitor-hook" where` prints it.

2. **Write the page** as `<folder>/<name>.html` (`<name>`: letters, digits, `-`, `_`). Read the template
   that fits and start from it — its comments are the markup reference:
   - `template-choice.html` — a decision: options drawn side by side, one question or a group;
   - `template-review.html` — a result for the user's OK: the render with what changed framed and
     numbered, «OK, commit it» / «Needs changes».

   Both are in this skill's folder (`~/.claude/skills/meownitor-round/`). The page links
   `round-kit.css` and `round-kit.js` by their bare names — the widget serves them, nothing to copy.
   Frames, numbers, screenshots, real-size drawings and the rest of the kit: `reference.md` there,
   when the page needs them.

   What makes a good round:
   - **draw** each option (HTML, inline SVG, canvas, a screenshot) — the real look, not a description;
     what is not the point as skeleton (`.sk`);
   - `data-rec` on the option you advise, with the reason; `data-now` on the current state;
   - options that could live side by side (a look the user may want to switch) → `data-multi`, or an
     option «make it a setting» — users often answer «B and D both» otherwise;
   - `p.why` under the question: the facts it rests on (`file:line`);
   - only what the user decides — technical choices they never see are yours;
   - the user's language, `<html lang>` to match; a short `<title>` (the widget row shows it).

3. **Ask** (Bash with `run_in_background: true`, no `timeout`):

   ```bash
   "${USERPROFILE:-$HOME}/.meownitor/bin/meownitor-hook" ask <name>
   ```

   It checks the page first: if it exits at once with a list of problems, fix the page and ask again.
   Otherwise tell the user in one line that the question is waiting in the widget and end the turn
   (or go on with other work). When they send, the waiter prints `ANSWER to <name>:` and the answer —
   that notification *is* the user's reply. Don't restart the waiter; it gives up after a day, and an
   answer sent after that comes with the user's next message.

   A page a project already made (with its own files inlined, its Send POSTing
   `{page: location.pathname, text}` to `/answer`): `ask <path>/<page>.html` copies it in and asks.

   If the user answers in the chat instead, or the question no longer matters, withdraw the round —
   the widget stops asking and the waiter exits:

   ```bash
   "${USERPROFILE:-$HOME}/.meownitor/bin/meownitor-hook" drop <name>
   ```

4. **Read the answer** — always this shape, whatever the page's language:

   ```
   <the page's title>
   1 — A
   2 — A, C
   3 — own option
     comment: …
   4 — no pick
   ```

   `own option` — no pick, the user wrote their own in the comment. «A②» in a comment is number 2 in
   option A's picture. Act on it; to ask again, ask only what is still open, in a new page with a new
   name — after «Needs changes», frame what changed since the last page.

The answer is also kept as `<name>.answer.md` next to the page. If the widget is not running, the
round waits until it is.
