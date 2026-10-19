# glimt

A resident dashboard for Linux that appears on a key press, in the shape of Raycast:
one process that stays in memory with its window built, so showing it is a map rather
than a start.

![The dashboard](docs/img/spike-raw.png)

It shows a calendar taken from [ugenr.dk] and a napkin calculator. The calendar is the
current week as seven coloured weekday boxes with its week number beside them, and a
half year laid out one month per column with ISO week numbers down the Mondays. The
calculator evaluates as you type, and a duration paints the days it covers.

## Keys

Two levels, told apart by whether anything has been typed.

| Key | While typing | Otherwise |
| --- | --- | --- |
| Escape | clear the input | dismiss the window |
| Ctrl+Q | clear it, return to today, dismiss | the same |
| up / down | move the selection a day | the same |
| Ctrl+up / down | move the selection a week | the same |
| anything printable | goes to the calculator | the same |

Neither key ends the process: the daemon stays resident, which is the point of it. It
exits on `glimtctl quit`.

## The calculator

```
2 + 3 * 4             14
13 usd in dkk         86,35 DKK
13 usd                86,35 DKK        kroner are the implied target
13 dkk                 1,74 EUR        kroner cannot be converted to kroner
today + 7 days        mandag 26. oktober 2026, and eight days painted on the grid
2026-10-19 + 30 days  onsdag 18. november 2026
2026-11-18 - 2026-10-19   30 dage, painted between the two
```

Whitespace is ignored, a comma is a decimal point, and words may be Danish or English.
Rates come from the European Central Bank's daily reference file, fetched at startup and
once a day, cached under `$XDG_CACHE_HOME/glimt/`, with a compiled-in table for the
first run and for being offline. An answer says which day's rates it used.

Half a typed expression is not an error, and is never reported as one: the previous
answer stays, greyed, and nothing else happens. `13 u` is on its way to `13 usd`, so it
says nothing; `13 xyzzy` cannot become anything, so it says so, quietly, once.

The parser is Rust and the semantics are Scheme. `crates/napkin-math/src/prelude.scm`
holds the value model and the dispatch of the four operators across numbers, amounts,
dates and durations, which is where another kind of napkin calculation would go.

## The two candidates

| Crate | Stack |
| --- | --- |
| `crates/spike-gtk4` | GTK4 with Pango, GSK and a style sheet |
| `crates/spike-raw` | smithay-client-toolkit, tiny-skia and cosmic-text, no toolkit |

Both are kept in step. Everything except the drawing is shared, so a feature is written
once and drawn twice: `glimt-cal` holds the calendar as data, `napkin-math` the
calculator, and `glimt-shell` the daemon, the D-Bus service, the global shortcut, the
design tokens and the state machine that says what a key does. Each frontend translates
its own key events into that vocabulary and renders the result.

The comparison and its numbers are in [docs/graphics-stack-decision.md].

## What the environment decides

GNOME's mutter does not implement `wlr-layer-shell` and has not for six years, so on
this desktop no application can own an always-on-top, exactly-placed, keyboard-exclusive
window. What is left is an ordinary toplevel, maximized so it fills the work area and
leaves the top bar alone, and the floor on how fast one can appear is mutter's own map
path rather than the toolkit's.

Two consequences shape both candidates:

- A window shown without an `xdg-activation-v1` token is refused focus and announced as
  a notification instead, so the shortcut comes from the GlobalShortcuts portal, whose
  `Activated` signal carries a token the compositor minted itself.
- Portability to a compositor that does have layer-shell is kept open in both: GTK
  through `gtk4-layer-shell`, and the raw candidate through smithay-client-toolkit.
  `just nested` checks that under a nested wlroots compositor on its headless backend,
  without leaving GNOME and without taking the screen.

## Running it

```sh
nix develop          # or: direnv allow
just dev             # the GTK candidate, visible
just dev spike-raw   # the other one

just dev spike-raw --input "13 usd in dkk"   # with something already typed
```

As a daemon, which is how it is meant to run:

```sh
just daemon spike-gtk4   # registers SUPER+space through the portal
just toggle              # or drive it by hand
```

## Measuring

```sh
just bench-all 200
just compare
```

Each run starts the candidate, waits for it, sits idle to measure what being resident
costs, then shows and hides it two hundred times. Latency is counted from the request
arriving to the compositor's own presentation timestamp, read from `wp_presentation` in
the raw candidate and from the frame clock in the GTK one.

[docs/graphics-stack-decision.md]: docs/graphics-stack-decision.md
[ugenr.dk]: https://www.ugenr.dk/
