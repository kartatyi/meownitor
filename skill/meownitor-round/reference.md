# The round kit — reference

What the kit (`round-kit.css`, `round-kit.js`) does by itself: the look in Dark and Light (the user's
switch), the question badge, option letters and the «recommended» / «current» pills, picking, the
comment field under each question, zoom, the bar with «Send» and the send — its own words in the
widget's language. The page holds only its content and its drawings' own styles.

## The page

- `<title>` — short: the widget's row, the window title and the answer's first line;
- `header.top` — `h1` and a `.sub` line or two: what we decide and what to look at;
- one `section.q` per question — `data-q` is its name in the answer (`1`, `Q1`; numbered in order when
  left out), `h2` the question, `p.why` the facts it is decided on; `data-multi` — several picks;
- one `.var` per option, drawn in `.vb`; `data-v` its letter (A, B… when left out); `data-rec` the one
  you advise, the reason in `.pm > .r`, a couple of short pluses and minuses (`.p`, `.m`); `data-now`
  the current state; class `wide` — the whole row. An option with nothing to draw is just `h3` (and a
  `p`) — a compact row.

Style a drawing by its own classes (`m-…`), not by the card round it: the zoom shows a copy of the
drawing outside its card.

## Show where to look: frames and numbers

A picture that holds more than the decision has to show where to look. Annotations are amber,
outlined and above the drawing — visibly not part of the UI.

- **A frame** when the picture holds a lot besides the part in question — a whole window, a full
  screen, a render of the app round one control: frame that part. In a drawing, draw what is not the
  point as skeleton (`.sk`); add `data-dim` to veil the rest when it still competes for the eye. A tag
  on the frame names it in a word or two.
- **Numbers** when the change lives in several places of one picture: a number on each, in reading
  order, and `ol.legend` with the same numbers saying what is at each; text refers to one as
  `<i class="mkn">2</i>`.
- **Both** when both hold: frame the area, number the places inside it.
- **Neither** for a small picture with one obvious thing in it. One or two frames and about seven
  numbers per picture at most — beyond that, split the picture.

How:

- **In a drawing** (HTML or SVG) put the attribute on the element itself; the kit draws the frame or
  the number at it and redraws on resize, theme and zoom: `data-frame` or `data-frame="new"` (a tag),
  with `data-dim`, `data-pad="6"` (the gap, 4 by default), `data-dash`; `data-mark="2"` with `data-at`
  — `tl` (default) `tr` `bl` `br`, or `l` `r` `t` `b` just outside the element, `c` on it.
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
| a screenshot as a file | copy the PNG/JPG/SVG into the round folder, `<img src="<name>-dark.png">` — never base64 written out |
| drawn at real size | `data-w="1440"` on `.vb` / `.fig` — laid out 1440 px wide, scaled down to fit, never up |
| a drawing that takes clicks | `data-live` on the visual — the zoom is then its corner button only |
| placing the drawing | `.vb.center`, `.fig.center`; `.bleed` — no padding |
| columns | three across, two for two or four options, fewer on a narrow window; `data-cols="2"` on `.vars` |
| a window to draw a screen in | `.win[data-title="…"]` |
| skeleton for what is not the point | `.sk` (a line; `style="--w:60%;--h:8px"`), `.sk.box`, `.sk.dot`, in `.sk-row` / `.sk-col` |
| colours for drawings | `--page --field --card --box --pop --line --line2 --ink --ink2 --ink3 --ink4 --fill0..2 --primary --success --warning --danger` (+ `-light`) — drawings that use them follow Dark/Light |
