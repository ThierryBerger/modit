#!/usr/bin/env python3
"""Render the wiring schematic for every modit module.

The **net tables in the per-module Markdown files are the source of truth**.
They are what `wiring_tables_match_firmware` (a test in `brain`) checks against
the firmware's pin constants, so a table and a firmware that disagree fail CI.
These drawings illustrate the tables; they define nothing.

Deliberately **no prose in the images**: every drawing carries designators and
values only, and the explanation lives in the Markdown next to it. Text baked
into a schematic cannot be searched, diffed, translated or read aloud, and it
collides with the circuit the moment a value gets longer.

One figure per sub-circuit, for the same reason -- two circuits sharing a
drawing grow into each other as soon as anything moves.

Run with `just schematics`. Writes `<name>.svg`, which is what the Markdown
references and what gets committed, plus `<name>.png`, which is gitignored and
exists only so a drawing can be checked by eye before committing.

Figures for a module that **does not exist yet** are written next to the plan that
designs it, in `plans/figures/`, not into `docs/hardware/`: this folder documents
modules you can build today, and a drawing with no page above it reads as a page
somebody deleted.
"""

from pathlib import Path

import schemdraw
import schemdraw.elements as elm

HERE = Path(__file__).parent
# Figures belonging to an unbuilt module, kept with the plan that designs it.
PLANNED = HERE.parent.parent / "plans" / "figures"
schemdraw.config(fontsize=12, lw=1.8)

FIGURES = {}


def figure(name):
    """Register a drawing under `name`, so `main` can render them all."""
    def wrap(fn):
        FIGURES[name] = fn
        return fn
    return wrap


def save(d, name, into=None):
    into = into or HERE
    into.mkdir(parents=True, exist_ok=True)
    for ext in ("svg", "png"):
        d.save(str(into / f"{name}.{ext}"), dpi=200)
    print(f"  {name} -> {into.name}/")


def pin(d, label, at, loc="left"):
    """A board pin, drawn as a terminal rather than as part of a block: these
    are wiring diagrams for a dev board, not PCB schematics."""
    t = elm.Dot(open=True).label(label, loc=loc).at(at)
    d += t
    return t


# ------------------------------------------------------------- button ------
@figure("module-button")
def module_button():
    with schemdraw.Drawing(show=False) as d:
        # The internal pull-down idles the pin LOW; SW1 pulls it UP to 3V3.
        pin(d, "3V3", (0, 0))
        d += elm.Button().right().label("SW1")
        d += elm.Line().right(1)
        pin(d, "GPIO33", d.here, loc="right")

        # GPIO26 high = lit.
        pin(d, "GPIO26", (0, -3))
        d += elm.Resistor().right().label("R1\n220 R - 1 k")
        d += elm.LED().right().label("D1")
        d += elm.Line().right(1)
        pin(d, "GND", d.here, loc="right")
        save(d, "module-button")


# --------------------------------------------------------------- coin ------
@figure("module-coin-input")
def module_coin_input():
    with schemdraw.Drawing(show=False) as d:
        # The acceptor's COIN output is open-collector, so the ESP32's internal
        # pull-up sets the idle level and 12 V never reaches this wire. R1 is
        # not part of the function: it survives a mis-identified wire.
        pin(d, "COIN\n(acceptor)", (0, 0))
        d += elm.Resistor().right().label("R1\n1 k")
        d += elm.Line().right(1)
        pin(d, "GPIO27", d.here, loc="right")

        # The grounds must be bonded -- drawn, because it is the one net that is
        # easy to leave out and impossible to work without.
        pin(d, "GND\n(acceptor PSU)", (0, -2))
        d += elm.Line().right(2).at((0, -2))
        d += elm.Ground().label("GND (ESP32)", loc="bottom")
        save(d, "module-coin-input")


@figure("module-coin-uno-input")
def module_coin_uno_input():
    with schemdraw.Drawing(show=False) as d:
        # R1 pulls up on the acceptor's side of R2, not the pin's. In normal
        # operation R2 then carries only the pin's leakage, so the pin reads the
        # acceptor's transistor directly however large R2 is -- which is what lets
        # R2 be 10 k, large enough to make a mis-identified 12 V wire harmless.
        pin(d, "COIN\n(acceptor)", (0, 0))
        d += elm.Line().right(1.5)
        d += (j := elm.Dot())
        d += elm.Resistor().right().label("R2\n10 k")
        d += elm.Line().right(1)
        pin(d, "D2", d.here, loc="right")

        d += elm.Resistor().up().label("R1\n10 k").at(j.center)
        d += elm.Vdd().label("5V")

        # The grounds are joined through the barrel jack; drawn anyway, because
        # it is the one net that is easy to leave out and impossible to work
        # without.
        pin(d, "GND\n(acceptor PSU)", (0, -2))
        d += elm.Line().right(2).at((0, -2))
        d += elm.Ground().label("GND (Uno)", loc="bottom")
        save(d, "module-coin-uno-input")


@figure("module-coin-uno-power")
def module_coin_uno_power():
    with schemdraw.Drawing(show=False) as d:
        # One supply, two loads, and two separate returns that meet only at the
        # supply's terminal: the solenoid's spike must not return through the
        # Uno's GND.
        pin(d, "12V\n(PSU +)", (0, 0))
        d += elm.Line().right(2)
        d += (plus := elm.Dot())
        d += elm.Line().right(3)
        pin(d, "DC12V (acceptor)", d.here, loc="right")
        d += elm.Line().down(1.2).at(plus.center)
        d += elm.Line().right(3)
        pin(d, "barrel centre (Uno)", d.here, loc="right")

        pin(d, "GND\n(PSU -)", (0, -3))
        d += elm.Line().right(2)
        d += (minus := elm.Dot())
        d += elm.Line().right(3)
        pin(d, "GND (acceptor)", d.here, loc="right")
        d += elm.Line().down(1.2).at(minus.center)
        d += elm.Line().right(3)
        pin(d, "barrel sleeve (Uno)", d.here, loc="right")
        save(d, "module-coin-uno-power")


# ------------------------------------------------------------- target ------
@figure("module-target-input")
def module_target_input():
    with schemdraw.Drawing(show=False) as d:
        # One piezo terminal to ground, the other into the conditioning network.
        d += (pz := elm.RBox(w=1.6).right().label("PIEZO").at((0, 0)))
        d += elm.Line().left(0.8).at(pz.start)
        d += elm.Line().down(1.2)
        d += elm.Ground()

        d += elm.Line().right(0.8).at(pz.end)
        d += (a := elm.Dot())

        # R1 bleeds the charge off and sets sensitivity.
        d += elm.Resistor().down().label("R1\n1 M").at(a.center)
        d += elm.Ground()

        # R2 limits the current the clamp diodes have to sink.
        d += elm.Resistor().right().label("R2\n100 k").at(a.center)
        d += (b := elm.Dot())

        # D1 clamps the positive swing to 3V3 + Vf.
        d += elm.Diode().up().label("D1\n1N4148").at(b.center)
        d += elm.Vdd().label("3V3")

        # D2 clamps the NEGATIVE swing, so its anode is at ground and its
        # cathode at the node. Drawn upwards FROM the ground end so the symbol
        # cannot come out reversed -- reversed, it would clamp the signal to
        # +0.7 V and quietly destroy it rather than protect the pin.
        d += elm.Line().down(2.6).at(b.center)
        d += (bot := elm.Dot())
        d += elm.Ground().at(bot.center)
        d += elm.Diode().up().label("D2\n1N4148", loc="bottom").at(bot.center)

        d += elm.Line().right(2.0).at(b.center)
        pin(d, "GPIO34\n(input-only, ADC1_CH6)", d.here, loc="right")
        save(d, "module-target-input", PLANNED)


@figure("module-target-led")
def module_target_led():
    with schemdraw.Drawing(show=False) as d:
        # Place the transistor first, then wire outwards from its own anchors.
        # Chaining off `d.here` past an anchored element leaves the leads
        # floating -- which looks almost right, and is not.
        d += (q := elm.NMos(bulk=False).at((5, 0)))
        d += elm.Dot().at(q.gate)

        d += elm.Resistor().left().label("R3\n100 R").at(q.gate)
        pin(d, "GPIO26", d.here)

        # R4 holds the gate low while the ESP32 boots and the pin floats.
        d += elm.Resistor().down().label("R4\n10 k", loc="bottom").at(q.gate)
        d += elm.Ground()

        # Long enough that the source ground clears R4's label.
        d += elm.Line().down(2.2).at(q.source)
        d += elm.Ground()

        d += elm.Line().up(0.8).at(q.drain)
        d += elm.RBox(w=2.0).up().label("LED strip\n12 V")
        d += elm.Vdd().label("+12 V")
        save(d, "module-target-led", PLANNED)


@figure("module-target-peakhold")
def module_target_peakhold():
    with schemdraw.Drawing(show=False) as d:
        pin(d, "node B\n(from R2)", (0, 0))
        d += elm.Diode().right().label("D3\n1N4148")
        d += (n := elm.Dot())

        d += elm.Capacitor().down().label("C1\n100 n").at(n.center)
        d += elm.Ground()

        d += elm.Line().right(1.6).at(n.center)
        d += (m := elm.Dot())
        d += elm.Resistor().down().label("R5\n1 M").at(m.center)
        d += elm.Ground()

        d += elm.Line().right(1.6).at(m.center)
        pin(d, "GPIO35\n(ADC1_CH7)", d.here, loc="right")
        save(d, "module-target-peakhold", PLANNED)


if __name__ == "__main__":
    print("rendering schematics:")
    for fn in FIGURES.values():
        fn()
    print(f"done -- {len(FIGURES)} figures")
