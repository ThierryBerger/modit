# Why modit exists

A fixed escape room can afford wires. It is built once, it lives in a building,
and you can run cable through the walls. Wireless props solve a problem that room
does not really have.

Modit is built for the case where there is no building. Or no time. Or no
permission to touch it.

## The constraint is setup, not wiring

The thing that makes a game hard to run outside a permanent installation is not
cable. It is everything you have to do before anyone can play:

- find power for each prop
- get them on a network, or build one
- put cable where people walk
- ask a venue for permission to do any of that
- and then take it all down again

Bluetooth LE removes all of it. A module needs power and nothing else. No router,
no venue wifi, no access point, no internet, no cable between boxes. You put the
boxes where you want them and turn them on.

That turns setup from an installation into unpacking a bag.

## Where that matters

**Sport training.** Reaction and agility drills with targets on a pitch. The
layout changes every session and often mid-session. Setup and teardown happen
around a training slot that is already too short.

**Outdoor and nomadic games.** A park, a field, a beach. There is no
infrastructure to plug into, and nothing to attach anything to.

**Conferences and trade shows.** A stand for one or two days, in a venue whose
wifi you cannot rely on and whose walls you cannot drill. It has to work when you
arrive and fit in a case when you leave.

**Museums.** Rooms that are protected, listed, or simply not yours to modify.
Cable is often the one thing you are not allowed to add.

**Festivals.** Temporary, outdoors, moved between days, and set up by whoever is
free.

## The fixed installation still works

None of this excludes the escape room. A permanent install is the easier case:
mains power, a fixed layout, time to set up once.

Designing for the nomadic case covers it. Designing for the fixed case first
would not have gone the other way — it would have produced something that assumes
a wall socket per prop and a network that is already there.

So the harder case is the target, and the easy one comes free.

## What follows from this

The framing is not decoration. It decides what a design here is allowed to
assume.

**Battery life is a requirement, not a feature.** In a fixed room you plug props
into the wall. On a pitch or in a park you do not, so a module has to earn a
session from a battery.

**A laptop in the loop is a setup step.** A laptop runs the game and talks to the
modules — one more thing to bring, charge, and keep in range. Nothing about the
protocol requires it, which is why the ideal remains "turn the boxes on".

**The number of modules changes per session.** Four cones today, seven next week,
one dies mid-drill. The system has to handle modules joining and leaving while a
game is running, rather than requiring a fixed roster at startup.

**Some applications are not pressed by a finger.** Sport training means hitting a
target with a ball: a different sensor, and an event that lasts milliseconds
instead of a tenth of a second.

## What this framing does not fix

Being honest about the cases where it is the wrong tool:

- **Range is about 10 m indoors**, less through walls. A museum spread over
  several rooms, or a festival field, will not all reach one brain. Modules
  passing a token between themselves helps, since each only needs to reach its
  neighbours, but it does not make the radio go further.
- **Simultaneous connections cap in the single digits to low double digits**, and
  throughput degrades before the cap. A handful of modules is fine.
- **Reaction timing has a jitter budget.** Measuring how fast someone responded
  means knowing when the light came on and when the hit landed. Bluetooth adds
  delay, and more importantly it adds *variable* delay. Reporting a split to the
  millisecond over a link with 100 ms of jitter would be a lie. Any sport
  application that claims timing has to measure that jitter first.
- **Unattended all-day operation is untested.** A conference stand runs for eight
  hours with nobody watching it. The firmware reboots after a panic and the brain
  re-acquires modules, but nothing has run that long yet.
