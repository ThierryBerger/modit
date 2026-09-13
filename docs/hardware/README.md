# Hardware conventions

How wiring is documented in this repository, and why it is documented this way.

Start from [`docs/HARDWARE.md`](../HARDWARE.md) -- this page is about the
*format*, not about any particular module.

## The net table is the source of truth

Every module page carries a table like this:

| Net | Pin | Direction | Connects to | Notes |
| --- | --- | --------- | ----------- | ----- |
| `BUTTON` | `GPIO33` | input, internal pull-down | `SW1` to `3V3` | rising edge = press |

**That table is the specification.** The schematic drawing next to it is an
illustration, and the prose underneath is an explanation. If the three ever
disagree, the table wins.

This is not a stylistic preference. A table:

- is **greppable** -- `grep GPIO33 docs/` answers "what is on that pin?" instantly;
- **diffs** meaningfully, so a pin change shows up in review as one line;
- is **checkable by a machine** -- see below;
- is readable by anything, including a screen reader, a terminal, and a language
  model asked to help with this project. An SVG is none of those things.

### It is checked against the firmware

`wiring_tables_match_firmware`, a test in `brain`, reads these tables *and* the
firmware source, and fails if the pins have drifted apart. So the documentation
cannot quietly go stale the way documentation normally does.

It is the same trick as `firmware_uuids_match_shared`, and it exists for the
same reason: the alternative is a comment that used to be true.

```
cargo test -p brain wiring_tables_match_firmware
```

If you change a pin in the firmware, that test tells you which table to update.

### A module that can be built on more than one board

When a module has more than one board -- `module-coin` has an Arduino Uno and an
ESP32 variant -- the sections that differ are wrapped in a tab block, one heading
per board:

```
<div class="tabs" data-group="board">

### Arduino Uno

| Net | Pin | ... |

### ESP32

| Net | Pin | ... |

</div>
```

On the website each block becomes tabs, and every block with the same
`data-group` switches together (`docs/site/tabs.js`). On GitHub the div is
dropped and the boards read one after the other. The blank lines inside the div
are required: without them the markdown inside is not rendered.

The check below reads only rows naming a `GPIOnn` pin, so an Uno's `Dn` rows are
**not checked yet**. A page with such a table says so.

## Designators

Standard component designators, numbered per module, not globally:

| Prefix | Meaning |
| ------ | ------- |
| `R` | resistor |
| `C` | capacitor |
| `D` | diode or LED |
| `Q` | transistor (bipolar or MOSFET) |
| `SW` | switch |
| `U` | integrated circuit |

Values are written the European way -- `100 R`, `4k7`, `1 M` -- because a decimal
point is the first thing to vanish from a photocopy or a hand-written label.

## The drawings

Schematics are generated from [`schematics.py`](schematics.py) using
[schemdraw](https://schemdraw.readthedocs.io).

**The `.svg` files are committed; the `.png` files are not.** SVG is text, so it
diffs, and GitHub renders it. PNGs are only produced so a drawing can be checked
by eye before committing, and `.gitignore` keeps them out. Reading the docs needs
no Python.

```
just schematics          # regenerate everything
```

Two rules about these drawings, both learned the hard way:

- **No prose inside an image.** Only designators and values. Text baked into a
  drawing cannot be searched, diffed or translated, and it collides with the
  circuit as soon as a value gets one character longer.
- **One figure per sub-circuit.** Two circuits in one drawing grow into each
  other the moment anything moves.

### Why not KiCad

KiCad is the right answer for a PCB, and if these modules ever become boards
instead of dev kits with flying leads, that is where they should go.

They are not boards. A modit module today is an ESP32 dev kit, three or four
discrete components, and some wire -- that is a *wiring harness*, not a PCB, and
a `.kicad_sch` is a binary-ish blob you cannot review in a terminal, cannot
diff usefully, and cannot regenerate from a script. The net table plus a
generated figure covers what actually gets built, and stays reviewable.

If a PCB happens, the net tables are what you would type into KiCad anyway.
