# Modit

**Games made of small wireless boxes you set up in minutes and pack away in
minutes.**

A modit module is a box with a light on it and something to hit. Give it power —
a USB battery pack will do — and it joins the game. There is no wire between
boxes, no venue wifi to borrow, no router to configure, and nothing to screw into
a wall.

Setup is putting the boxes where you want them and turning them on.

## What a game made of them looks like

A laptop runs the game and talks to the boxes over Bluetooth. The game is a short
program, and it is the part you write:

> Ten boxes along a pitch. One lights up. A player sprints to it and hits it; the
> next one lights. Ten laps, every split timed, the slowest one called out at the
> end.

That is one of the four games that ship with modit. The others are whack-a-mole,
a memory game, and a coin-operated one where a coin buys a round. Swapping
between them is a different program against the same boxes — which is the idea:
the boxes are generic, the game is not.

**You can play all four right now without owning any hardware**, because the same
program runs against a simulator. See [getting started](GETTING-STARTED.md).

## Who it is for

Anyone running a game where there is no building to wire, no time to install, or
no permission to drill: sport coaches doing reaction and agility drills, outdoor
and nomadic games, a conference stand up for two days, a museum room that is not
yours to modify, a festival.

A fixed escape room works too — it is the easier case, and designing for the hard
one covered it. The reasoning, and what it decided about the project, is in
[why modit exists](WHY.md).

## What exists today

This is an early, honest-about-itself project rather than a product.

One module type — **a button and a light** — works on real hardware. A **coin
acceptor** module is written and plays in simulation but has not been attached to
a real acceptor yet.

Each module's page says which of the two it is, and
[hardware](HARDWARE.md) lists them in one table. Nothing here is described as
working when it has not met a board.

It will not solve every case: Bluetooth reaches about 10 m indoors, a handful of
modules is comfortable where fifteen needs thought, and nothing has yet been run
unattended for a whole day. The limits are written out, with reasons, at the end
of [why modit exists](WHY.md).

## What it costs

Per module: an ESP32 development board, a button, an LED and a resistor — a few
euros, and no custom circuit board. The bill of materials for each module is on
its own page under [hardware](HARDWARE.md). The coin acceptor is the expensive
one, and the only one that needs a 12 V supply.

## Where to go

| If you want to | Read |
| -------------- | ---- |
| play a game in the next five minutes, with no hardware | [Getting started](GETTING-STARTED.md) |
| know whether this suits what you are building | [Why modit exists](WHY.md) |
| design a game out of modules | [Composing a game](COMPOSING.md) |
| build a module | [Hardware](HARDWARE.md) |
| follow the wiring and flashing step by step | [Tutorial](TUTORIAL.md) |
| change modit itself | [Architecture](ARCHITECTURE.md), then [plans](../plans/README.md) |
