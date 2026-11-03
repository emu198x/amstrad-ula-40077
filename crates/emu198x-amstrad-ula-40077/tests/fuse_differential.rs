//! Compare physical WAIT memory cycles with FUSE's independently declared
//! maximum delay across a complete frame. The phase-resolved unit test also
//! checks all eight falling-edge arrivals; a maximum alone cannot qualify it.
//!
//! Reference: vendored FUSE 1.7.0 `spectrum.c`, `contention_pattern_76543210`
//! and `contend_delay_common(time, pattern, 4)`; libspectrum 1.6.0
//! `timings_frame_amstrad_asic`. Hardware uses WAIT, as specified in the
//! Amstrad +3 manual p. 273, rather than suppressing the CPU clock.

use emu198x_amstrad_ula_40077::AmstradGateArray;
use emu198x_bus_sinclair_zx_spectrum::MemoryBus;
use emu198x_bus_sinclair_zx_spectrum::Ula;
use emu198x_bus_sinclair_zx_spectrum::timing::{SCREEN_HEIGHT, SCREEN_WIDTH};

const PER_LINE: u32 = 228;
const DISPLAY_LINES: u32 = 192;
const BORDER_HEIGHT: u32 = 48;
const LEFT_BORDER: u32 = 24;
const HORIZONTAL_SCREEN: u32 = 128;
const TOP_LEFT_PIXEL: u32 = 14_365;
const OFFSET: u32 = 4;
const FUSE_PATTERN: [u32; 8] = [5, 4, 3, 2, 1, 0, 7, 6];

/// `machine.c` sets `line_times[0] = top_left_pixel - border*per_line - 16`.
const LINE_TIMES_0: u32 = TOP_LEFT_PIXEL - BORDER_HEIGHT * PER_LINE - 16;

struct Contended;

impl MemoryBus for Contended {
    fn read(&self, _addr: u16) -> u8 {
        0
    }

    fn write(&mut self, _addr: u16, _value: u8) {}

    fn is_contended(&self, _addr: u16) -> bool {
        true
    }
}

/// FUSE's `contend_delay_common` for the Amstrad ASIC.
fn fuse_delay(t: u32) -> u32 {
    if t < LINE_TIMES_0 {
        return 0;
    }
    let line = (t - LINE_TIMES_0) / PER_LINE;
    if !(BORDER_HEIGHT..BORDER_HEIGHT + DISPLAY_LINES).contains(&line) {
        return 0;
    }
    let through = (t - LINE_TIMES_0 + (LEFT_BORDER - 16)) % PER_LINE;
    if !(LEFT_BORDER - OFFSET..LEFT_BORDER + HORIZONTAL_SCREEN - OFFSET).contains(&through) {
        return 0;
    }
    FUSE_PATTERN[(through % 8) as usize]
}

/// A three-T-state memory read, with complete TW pairs at T2 falling.
fn mcycle_cost(gate: &mut AmstradGateArray, framebuffer: &mut [u8]) -> u32 {
    let mut phase = 0usize;
    let mut ticks = 0usize;
    while phase < 6 {
        assert!(ticks < 128, "memory cycle never completed");
        let mreq = (1..=5).contains(&phase);
        Ula::tick(gate, &Contended, 0x4000, mreq, false, false, framebuffer);
        assert!(gate.cpu_clock_active(), "WAIT must not suppress clocks");
        ticks += 1;
        if phase == 3 && gate.wait_active() {
            phase = 2;
        } else {
            phase += 1;
        }
    }
    assert_eq!((ticks - 6) % 2, 0, "WAIT must add complete T-states");
    ((ticks - 6) / 2) as u32
}

/// Probe every arrival in one frame from a free-running beam. Back-to-back
/// memory cycles sample a synchronized subset of phases and can miss the
/// longest wait entirely, so they cannot establish the frame's maximum.
fn mcycle_costs() -> Vec<u32> {
    let mut gate = AmstradGateArray::new();
    let mut framebuffer = vec![0; SCREEN_WIDTH * SCREEN_HEIGHT];
    let frame_tstates = gate.frame_timing().tstates_per_frame;
    for _ in 0..frame_tstates * 2 {
        Ula::tick(
            &mut gate,
            &Contended,
            0x4000,
            false,
            false,
            false,
            &mut framebuffer,
        );
    }
    let mut costs = Vec::new();
    for _ in 0..frame_tstates {
        let mut probe = gate.clone();
        costs.push(mcycle_cost(&mut probe, &mut framebuffer));
        for _ in 0..2 {
            Ula::tick(
                &mut gate,
                &Contended,
                0x4000,
                false,
                false,
                false,
                &mut framebuffer,
            );
        }
    }
    costs
}

#[test]
fn contention_matches_fuse_across_the_whole_frame() {
    let costs = mcycle_costs();
    let engine_max = costs.iter().copied().max().unwrap_or(0);
    let fuse_max = (0..70_908u32).map(fuse_delay).max().unwrap_or(0);
    let contended = costs.iter().filter(|&&c| c > 0).count();

    println!(
        "\n+2A M-cycles: {} measured, {contended} contended, \
         engine max delay {engine_max} T-states, FUSE max {fuse_max}",
        costs.len(),
    );

    // Self-check first: an instrument that cannot reproduce a known-good
    // measurement is not measuring the thing it claims to. The wind-in
    // probe in #856 sees stalls, so a drive that sees none is broken.
    assert!(
        engine_max > 0,
        "harness fault, not a finding: this drive measured no contention \
         anywhere in a frame. Fix the drive before reading any comparison."
    );

    assert_eq!(
        engine_max, fuse_max,
        "the +2A gate array tops out at {engine_max} T-states of contention \
         against FUSE's {fuse_max}."
    );
}
