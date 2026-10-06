---
name: meownitor-round
description: Ask the user a question with visual options through Meownitor — one HTML page (a short description and one question or a group of them, the options drawn side by side) that the user answers in a window on their desktop; the answer comes back to you on its own. Use when a decision is easier made by seeing the options — UI looks, layouts, designs, diagrams, anything visual or rich — when you show a result for the user's OK (a render, a screen) with what changed framed and numbered, or when the user asks to be asked visually.
---

# Ask through the widget

Meownitor (the pixel cat on the user's desktop) shows your session as asking, chirps once,
and opens your page in a window centred on the user's monitor when they press «Answer». Their
answer reaches you through a background waiter — no polling, no copy-paste.

## Steps

1. **The round folder and the kit** (Bash) — one command; it prints this session's round folder
   (from `CLAUDE_CODE_SESSION_ID`, which Claude Code sets) and puts the kit next to the page:

   ```bash
   k="${USERPROFILE:-$HOME}/.claude/skills/meownitor-round"   # this skill's folder
   d=$("${USERPROFILE:-$HOME}/.meownitor/bin/meownitor-hook" where) && cp "$k/round-kit.css" "$k/round-kit.js" "$d/" && echo "$d"
   ```

2. **Write the page** as `<folder>/<name>.html` — `<name>` is letters, digits, `-` and `_`. Start from a
   template in this skill's folder (read it first; its comments are the markup reference):
   - `template-choice.html` — a decision: the variants drawn side by side, one question or a group;
   - `template-review.html` — a result for the user's OK: the screen or render with what changed
     framed and numbered, «OK, commit it» / «Needs changes».

   The page links `round-kit.css` and `round-kit.js` and holds only its content and its drawings' own
   styles; the kit does the rest — the look in Dark and Light (the user's switch), the question badge,
   the variant letters and the «recommended» pill, picking, the comment field, zoom, the bar with «Send»
   and the send itself — its own words in the widget's language. What goes on it:
   - `<title>` — short; it is what the widget row and the window title show;
   - `header.top` — `h1` and a `.sub` line or two: what we decide and what to look at;
   - one `section.q` per question (`data-q` = its name in the answer, `1` or `Q1`); `h2` the question,
     `p.why` the facts it is decided on (what code or firmware dictates, with `file:line`);
   - one `.var` per option, **drawn** in `.vb` (HTML, inline SVG, canvas or a screenshot — the real look,
     not a description); `data-rec` on the one you advise with the reason in `.pm > .r`, a couple of short
     pluses and minuses (`.p`, `.m`); `data-now` on the current state; class `wide` for a big option that
     needs the whole row. An option with nothing to draw is just `h3` (and a `p`) — a compact row;
   - several picks allowed: `data-multi` on the `.q`.

   The page's text in the user's language, with `<html lang>` to match. Technical choices the user
   never sees are yours to make — don't ask them. A page a project already made can go too — as a
   self-contained copy with its own files inlined, as long as its send POSTs
   `{page: location.pathname, text}` to `/answer`.

3. **Wait in the background** (Bash with `run_in_background: true`):

   ```bash
   "${USERPROFILE:-$HOME}/.meownitor/bin/meownitor-hook" wait <name>
   ```

   Then tell the user in one line that the question is waiting in the widget, and end the turn (or go
   on with other work). When they send, the waiter prints `ANSWER to <name>:` followed by their answer
   and exits — that notification *is* the user's reply.

   The user answers when they get to it: don't put a `timeout` on the waiter, and don't restart it.
   It gives up by itself after a day. An answer sent after it stopped — gave up, was stopped, or went
   with a restart of the session — comes with the user's next message, in the same shape.

   If the user answers in the chat instead, take it from there and withdraw the round, so the widget
   stops asking — it hides the question and closes its window, and the waiter exits by itself:

   ```bash
   "${USERPROFILE:-$HOME}/.meownitor/bin/meownitor-hook" drop <name>
   ```

   Withdraw a round the same way when it no longer matters — the question changed, or you are asking
   it again in a new page.

4. **Read the answer** — always in this shape, whatever the page's language: the page's title, then one
   line per question — `1 — A`, `2 — A, C` with several picks, `1 — no pick` — and `  comment: …` under
   it when they wrote one. «A②» in a comment points at number 2 in option
   A's picture (a click on a number puts it there). Act on it; if you ask again, ask only what is still
   open, in a new page with a new name.

The answer is also kept next to the page as `<name>.answer.md`. If the widget is not running, the
round simply waits until it is.

## Show where to look: frames and numbers

A picture that holds more than the decision has to show where to look. Annotations are amber,
outlined and above the drawing — visibly not part of the UI.

- **A frame** when the picture holds a lot besides the part in question — a whole window, a full
  screen, a render of the app round one control: frame that part. In a drawing, draw what is not the
  point as skeleton (`.sk`) instead of detail; add `data-dim` to veil the rest when it still competes
  for the eye (a busy screen, a real render). A tag on the frame names it in a word or two.
- **Numbers** when the change lives in several places of one picture: a number on each place, in
  reading order (left to right, top to bottom), and `ol.legend` with the same numbers saying what is
  at each; text refers to one as `<i class="mkn">2</i>`.
- **Both** when both hold: frame the area, number the places inside it.
- **Neither** for a small picture with one obvious thing in it. One or two frames and about seven
  numbers per picture at most — beyond that, split the picture.
- In a round that follows «Needs changes», frame what changed since the previous page, so the eye goes
  straight to it.

How:

- **In a drawing** (HTML or SVG) put the attribute on the element itself; the kit draws the frame or
  the number at it and redraws on resize, theme and zoom:
  `data-frame` or `data-frame="new"` (a tag), with `data-dim`, `data-pad="6"` (the gap, 4 by default),
  `data-dash`; `data-mark="2"` with `data-at` — `tl` (default) `tr` `bl` `br`, or `l` `r` `t` `b`
  just outside the element, `c` on it.
- **On a screenshot** (`.shot` holding the `<img>`): `<i class="frame" data-px="x y w h" data-label="new">`
  and `<i class="mkn" data-px="x y">2</i>`, in the image's own pixels — or in percent of it,
  `style="left:41%;top:22%;width:30%;height:12%"`.
- A note on a drawing that is not a number: `<span class="anno">8 px</span>`, a dashed amber tag.

## The kit's other pieces

| what | markup |
|---|---|
| a checked fact under the question | `.fact` (`.fact.warn` — amber edge) |
| a question's own picture | `.fig`; beside its legend: `.with-legend > .fig + ol.legend` |
| a render per theme | `.shot > img.only-dark + img.only-light` — the page shows the one matching its theme |
| a screenshot as a file | copy the PNG/JPG/SVG into the round folder and use `<img src="<name>-dark.png">` — no base64 to write out |
| drawn at real size | `data-w="1440"` on `.vb` / `.fig` — laid out 1440 px wide, scaled down to fit, never up |
| a drawing that takes clicks | `data-live` on the visual — the zoom is then its corner button only |
| placing the drawing | `.vb.center`, `.fig.center`; `.bleed` — no padding |
| columns | three across, two for two or four options, fewer on a narrow window; `data-cols="2"` on `.vars` |
| a window to draw a screen in | `.win[data-title="…"]` |
| skeleton for what is not the point | `.sk` (a line; `style="--w:60%;--h:8px"`), `.sk.box`, `.sk.dot`, in `.sk-row` / `.sk-col` |
| colours for drawings | `--page --field --card --box --pop --line --line2 --ink --ink2 --ink3 --ink4 --fill0..2 --primary --success --warning --danger` (+ `-light`) — drawings that use them follow Dark/Light |

Style a drawing by its own classes (`m-…`), not by the card round it: the zoom shows a copy of the
drawing outside its card.
