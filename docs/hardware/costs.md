# Module costs — draft to fill in

Estimates are in euros, 2026, single units. The first figure is the cheapest
realistic source (AliExpress and similar), the second a fast one (Amazon.fr, a
French electronics shop). Fill in **Paid** (and **Where** if you remember), and
answer the questions under each table. Parts sold in packs: put the pack price
and the pack size, e.g. `4.50 / 100`.

## module-button

| Qty | Part | Estimate | Paid | Where |
| --- | ---- | -------- | ---- | ----- |
| 1 | ESP32 dev board (plain ESP32, Xtensa) | 4 – 10 | | |
| 1 | `SW1` arcade button, 100 mm, with built-in light | 6 – 15 | | |
| 1 | Case: junction box (boîte de dérivation), drilled for the button | 2 – 5 | | |
| 1 | `R1` resistor, 220 R – 1 k | ~0.02 (kit: 5 – 10) | | |
| 1 | `D1` LED (if not using the button's own light) | ~0.05 | | |
| 1 | USB data cable | 2 – 5 | | |
| — | Jumper wires / breadboard / connectors | 3 – 8 | | |
| — | **Total** | **~17 – 45** | | |

Questions:

1. What voltage is the arcade button's light rated for (5 V, 12 V)? Is it an
   LED with a built-in resistor, or a bulb? A 12 V light cannot be driven
   straight from `GPIO26`: it would need a transistor, which changes the BOM.
2. Did the button come with its microswitch, or was that separate?
3. What size is the junction box, and does anything hold the ESP32 inside it
   (standoffs, glue, foam)?
4. How is it powered away from the laptop: a USB battery pack, a charger? If a
   battery pack, its price.
5. Do you still use the small tactile switch on a breadboard, or is the arcade
   button now the only build?

## module-coin — Arduino Uno

| Qty | Part | Estimate | Paid | Where |
| --- | ---- | -------- | ---- | ----- |
| 1 | Arduino Uno clone (Kuman Uno) | 10 – 18 | | |
| 1 | Coin acceptor, CH-92x family (616) | 10 – 25 | | |
| 1 | 12 V PSU, 5.5 × 2.1 mm, ≥ 1 A | 6 – 12 | | |
| 1 | DC splitter or barrel-to-screw-terminal adapter | 1 – 4 | | |
| 2 | `R1`, `R2` resistors, 10 k | ~0.04 | | |
| 1 | USB cable (USB-B, for the Uno) | 2 – 4 | | |
| — | Wires / connectors | 2 – 5 | | |
| — | **Total** | **~31 – 68** | | |

Questions:

6. Did the Uno come in a kit (with cables, breadboard, resistors)? If so, the
   kit price and what was in it.
7. Is the acceptor in a case or mounted on anything yet?

## module-coin — ESP32

| Qty | Part | Estimate | Paid | Where |
| --- | ---- | -------- | ---- | ----- |
| 1 | ESP32 dev board | 4 – 10 | | |
| 1 | Coin acceptor, CH-92x family | *shared with the Uno build* | | |
| 1 | 12 V PSU | *shared with the Uno build* | | |
| 1 | `R1` resistor, 1 k | ~0.02 | | |
| 1 | USB data cable | 2 – 5 | | |
| — | **Total (acceptor + PSU included)** | **~22 – 57** | | |

## Shared / one-off

| Part | Estimate | Paid | Where |
| ---- | -------- | ---- | ----- |
| Brain: BLE adapter, if not built into the laptop | 5 – 15 | | |
| Resistor assortment kit | 5 – 10 | | |
| LED assortment kit | 3 – 6 | | |
| Jumper wire kit | 3 – 8 | | |
| Multimeter | 10 – 30 | | |
| Soldering iron / consumables | 15 – 60 | | |

Questions:

8. Shipping: roughly how much, and should it be counted per part or left out?
9. Anything bought that is not listed here (heat-shrink, drill bit for the box,
   screws, cable glands)?
