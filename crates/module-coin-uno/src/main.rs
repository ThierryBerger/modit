//! Firmware for a coin acceptor on an Arduino Uno, standalone.
//!
//! Counts the pulses a CH-92x-family acceptor emits for each coin and blinks the
//! on-board `L` LED once per pulse, one group per coin. It has no radio and does
//! not talk to the brain.
//!
//! The counting, debouncing and burst-settling rules are the ones every coin
//! firmware uses, from [`shared::input`] and [`shared::coin`]. This crate supplies
//! a millisecond clock, the interrupt, and the LED.
//!
//! # Wiring and power
//!
//! Read `docs/hardware/module-coin.md`, Arduino Uno tabs, before connecting
//! anything. In short:
//!
//! - **`D2`** (`INT0`) is the COIN pulse line, through `R2`, with `R1` pulling
//!   the acceptor's side of it up to 5 V. The internal pull-up stays **off**.
//! - **`D13`** is the on-board `L` LED.
//! - The Uno runs from the acceptor's 12 V supply through its barrel jack. **USB
//!   and the 12 V supply are never plugged in at the same time.**
//!
//! # What the LED says
//!
//! | Pattern | Meaning |
//! | ------- | ------- |
//! | three slow blinks | booted (also what a reset looks like) |
//! | fast blinking, forever | `D2` read low at rest: nothing is counted |
//! | *n* quick blinks in a group | a coin that emitted *n* pulses |
//!
//! # Flashing
//!
//! `just flash-coin-uno` from the repository root, with the 12 V supply unplugged.

#![no_std]
#![no_main]
#![feature(abi_avr_interrupt)]

use arduino_hal::{
    port::{mode::Output, Pin},
    prelude::*,
};
use panic_halt as _;
use portable_atomic::{AtomicU32, Ordering};
use shared::coin::{BURST_GAP_MS, PULSE_DEBOUNCE_MS};
use shared::input::{saturating_u8, EdgeLatch};

/// On and off time of each self-test blink.
const SELF_TEST_BLINK_MS: u32 = 500;
/// Toggle period of the "`D2` reads low" alarm.
const FAULT_BLINK_MS: u32 = 100;
/// On and off time of each per-pulse blink. Fast enough that a coin's group is
/// over before the next coin can be recognised.
const PULSE_BLINK_MS: u32 = 120;

/// Milliseconds since `TC0` was started.
static MILLIS: AtomicU32 = AtomicU32::new(0);

/// Pulses counted by `INT0` but not yet blinked.
static PULSES: EdgeLatch = EdgeLatch::new();

/// Start `TC0` interrupting once per millisecond.
///
/// CTC mode: 16 MHz / 64 is 250 kHz, and counting 0 to 249 is 250 ticks.
fn millis_init(tc0: arduino_hal::pac::TC0) {
    tc0.tccr0a().write(|w| w.wgm0().ctc());
    tc0.ocr0a().write(|w| w.set(249));
    tc0.tccr0b().write(|w| w.cs0().prescale_64());
    tc0.timsk0().write(|w| w.ocie0a().set_bit());
}

fn millis() -> u32 {
    MILLIS.load(Ordering::Relaxed)
}

#[avr_device::interrupt(atmega328p)]
fn TIMER0_COMPA() {
    MILLIS.fetch_add(1, Ordering::Relaxed);
}

// One falling edge on `D2`: one pulse, unless it is bounce.
#[avr_device::interrupt(atmega328p)]
fn INT0() {
    PULSES.record(millis(), PULSE_DEBOUNCE_MS);
}

fn blink(led: &mut Pin<Output>, times: u8, ms: u32) {
    for _ in 0..times {
        led.set_high();
        arduino_hal::delay_ms(ms);
        led.set_low();
        arduino_hal::delay_ms(ms);
    }
}

#[arduino_hal::entry]
fn main() -> ! {
    let dp = arduino_hal::Peripherals::take().unwrap();
    let pins = arduino_hal::pins!(dp);
    let mut serial = arduino_hal::default_serial!(dp, pins, 57600);
    let mut led = pins.d13.into_output().downgrade();
    // Floating: `R1`, on the acceptor's side of `R2`, is the pull-up.
    let coin = pins.d2.into_floating_input();

    millis_init(dp.TC0);
    // SAFETY: not called from inside a critical section.
    unsafe { avr_device::interrupt::enable() };

    ufmt::uwriteln!(
        &mut serial,
        "modit coin module (Uno): burst gap {} ms, debounce {} ms\r",
        BURST_GAP_MS,
        PULSE_DEBOUNCE_MS
    )
    .unwrap_infallible();

    // Also gives the acceptor, powered up at the same instant, time to settle
    // before its line is judged.
    blink(&mut led, 3, SELF_TEST_BLINK_MS);

    // Open collector pulled up by `R1`: released, the line reads high.
    if coin.is_low() {
        ufmt::uwriteln!(
            &mut serial,
            "self-test: WARNING -- D2 reads LOW at rest. Expected high. Either the \
             NO/NC switch is on NC, the grounds are not joined, or this is not the \
             COIN wire. Counting nothing.\r"
        )
        .unwrap_infallible();
        loop {
            led.toggle();
            arduino_hal::delay_ms(FAULT_BLINK_MS);
        }
    }
    ufmt::uwriteln!(&mut serial, "self-test: D2 idles high; post a coin\r").unwrap_infallible();

    // Falling edge only: the acceptor pulls the line low for each pulse, and
    // one pulse must count once. The stale flag is cleared before arming.
    dp.EXINT.eicra().modify(|_, w| w.isc0().set(0b10));
    dp.EXINT.eifr().write(|w| w.intf().set(0b01));
    dp.EXINT.eimsk().modify(|_, w| w.int0().set_bit());

    loop {
        if let Some(pulses) = PULSES.take_settled(millis(), BURST_GAP_MS) {
            let pulses = saturating_u8(pulses);
            ufmt::uwriteln!(&mut serial, "coin: {} pulse(s)\r", pulses).unwrap_infallible();
            blink(&mut led, pulses, PULSE_BLINK_MS);
        }
    }
}
