//! Amstrad 40077 gate array — the +2A / +2B / +3 ZX Spectrum's custom chip.
//!
//! Source references:
//! - `knowledge/chips/amstrad-40077.md`
//! - `knowledge/systems/spectrum/contention.md`
//! - Adapted from `../Emu198x-Older/crates/amstrad-ula-40077/src/lib.rs`
//!
//! Key differences from the Sinclair ULAs:
//! - Different contention pattern: `[1, 0, 7, 6, 5, 4, 3, 2]`
//! - **No I/O contention** — the gate array is MREQ-only
//! - **No internal-op contention** — MREQ is not active during internal cycles
//! - Different contended-bank rules at `$C000` (banks 4–7, not odd banks)
//! - **No floating bus** — reads from unattached ports always return `$FF`
//!
//! Same crystal and line layout as the 128K (17.734475 MHz, 228 T/line, 311 lines).

use emu198x_bus_sinclair_zx_spectrum::MemoryBus;
use emu198x_bus_sinclair_zx_spectrum::Ula;
use emu198x_bus_sinclair_zx_spectrum::timing::{self, FrameTiming};
use emu198x_sinclair_zx_spectrum_ula::{self as ula_engine, DELAY_TABLE_PLUS2A, UlaEngine};

/// Amstrad 40077 gate array.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct AmstradGateArray {
    engine: UlaEngine,
}

impl AmstradGateArray {
    #[must_use]
    pub fn new() -> Self {
        Self {
            engine: UlaEngine::new(&ula_engine::CONFIG_PLUS2A),
        }
    }

    #[must_use]
    pub fn border_color(&self) -> u8 {
        self.engine.border
    }

    /// Reinstall the +2A/+3 timing config after a snapshot restore.
    ///
    /// `UlaEngine::config` is `#[serde(skip)]` and falls back to the
    /// 48K config on deserialise (see `emu198x_sinclair_zx_spectrum_ula::
    /// default_config`). The Amstrad gate-array uses
    /// CONFIG_PLUS2A — different contention pattern, different line
    /// length. Call once after `restore`.
    pub fn reattach_config(&mut self) {
        self.engine.set_config(&ula_engine::CONFIG_PLUS2A);
    }
}

impl Default for AmstradGateArray {
    fn default() -> Self {
        Self::new()
    }
}

impl Ula for AmstradGateArray {
    fn tick(
        &mut self,
        memory: &dyn MemoryBus,
        cpu_addr: u16,
        cpu_mreq: bool,
        cpu_iorq: bool,
        // The Amstrad 40077/40078 gate array is not (yet) modelled as
        // exhibiting the Ferranti/Sinclair snow effect, so the refresh
        // line is unused here.
        _cpu_rfsh: bool,
        framebuffer: &mut [u8],
    ) {
        let e = &mut self.engine;
        let phase = (e.pixel as usize) & 0x0F;

        e.tick_rendering(memory, framebuffer, None);

        // Amstrad contention: MREQ-only. No I/O contention, no internal
        // contention. The gate array contends only when the CPU is mid
        // memory access (MREQ asserted) and the address is in a
        // contended range. This is the inverse of the Sinclair model
        // (which contends when MREQ is *not* yet active to gate the
        // upcoming clock).
        if e.video {
            let contended_addr = memory.is_contended(cpu_addr);
            let contention = contended_addr && cpu_mreq && e.z80_clock_high;
            e.cpu_clock = !(contention && DELAY_TABLE_PLUS2A[phase]);
        } else {
            e.cpu_clock = true;
        }

        e.track_z80_clock(cpu_iorq, cpu_mreq, cpu_iorq && (cpu_addr & 1) == 0);
    }

    fn cpu_clock_active(&self) -> bool {
        self.engine.cpu_clock
    }

    fn interrupt_active(&self) -> bool {
        self.engine.int_active
    }

    fn floating_bus(&self) -> u8 {
        // The Amstrad gate array does not expose the data bus on
        // unattached ports — reads always return $FF.
        0xFF
    }

    /// With no tape signal, bit 6 reads 0 whatever was last written.
    ///
    /// Unlike the Sinclair ULAs, the 40077 takes MIC out on pin 98 and
    /// EAR in on pin 2, and the board keeps them apart (+2A/+3 circuit
    /// diagram, issue 3, 26/1/87). The EAR pin is fed by TR4, a
    /// common-emitter stage whose base R66 (10K) pulls up to +5V, so at
    /// rest TR4 conducts and holds the pin low. The writes to bits 3 and 4
    /// that set the idle level on a 48K never reach it. FUSE reads `$BF`
    /// on these machines for the same reason.
    fn read_fe(&self, port: u16, keyboard: &[u8; 8]) -> u8 {
        self.engine.read_fe(port, keyboard) & !0x40
    }

    fn write_fe(&mut self, val: u8) {
        self.engine.write_fe(val);
    }

    fn frame_timing(&self) -> &FrameTiming {
        &timing::TIMING_PLUS2A
    }

    fn end_frame(&mut self) {
        self.engine.end_frame();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_timing_matches_plus2a_constants() {
        let g = AmstradGateArray::new();
        let t = g.frame_timing();
        assert_eq!(t.master_hz, 17_734_475);
        assert_eq!(t.cpu_divisor, 5);
        assert_eq!(t.tstates_per_line, 228);
        assert_eq!(t.lines_per_frame, 311);
        assert_eq!(t.tstates_per_frame, 70_908);
        assert_eq!(t.contention_pattern, [1, 0, 7, 6, 5, 4, 3, 2]);
        assert_eq!(t.contention_phase, 0);
    }

    #[test]
    fn floating_bus_always_returns_high() {
        let g = AmstradGateArray::new();
        assert_eq!(g.floating_bus(), 0xFF);
    }

    #[test]
    fn defaults_to_white_border() {
        let g = AmstradGateArray::new();
        assert_eq!(g.border_color(), 7);
        assert!(!g.interrupt_active());
    }

    #[test]
    fn write_fe_updates_border() {
        let mut g = AmstradGateArray::new();
        g.write_fe(0x02); // border = red (0b010)
        assert_eq!(g.border_color(), 2);
    }

    /// Memory whose contention answer the test chooses.
    struct Memory {
        contended: bool,
    }

    impl MemoryBus for Memory {
        fn read(&self, _addr: u16) -> u8 {
            0
        }

        fn write(&mut self, _addr: u16, _value: u8) {}

        fn is_contended(&self, _addr: u16) -> bool {
            self.contended
        }
    }

    const CONTENDED: Memory = Memory { contended: true };
    const UNCONTENDED: Memory = Memory { contended: false };

    /// Half-cycle ticks in one +2A/+3 frame: 456 pixels by 311 lines.
    const FRAME_TICKS: usize = 456 * 311;

    /// Tick index (from the start of a frame) on which the interrupt rises:
    /// line 248, pixel 1 of `CONFIG_PLUS2A`.
    const INT_RISE_TICK: usize = 248 * 456 + 1;

    fn framebuffer() -> Vec<u8> {
        vec![0; timing::SCREEN_WIDTH * timing::SCREEN_HEIGHT]
    }

    /// Tick a whole frame with fixed pins and count the half-cycles on
    /// which the gate array withholds the CPU clock.
    fn stalls_over_a_frame(memory: &Memory, addr: u16, mreq: bool, iorq: bool) -> usize {
        let mut g = AmstradGateArray::new();
        let mut fb = framebuffer();
        let mut stalls = 0;
        for _ in 0..FRAME_TICKS {
            Ula::tick(&mut g, memory, addr, mreq, iorq, false, &mut fb);
            if !g.cpu_clock_active() {
                stalls += 1;
            }
        }
        stalls
    }

    /// Ticks from now until the interrupt line rises, and how long it stays up.
    fn interrupt_rise_and_width(g: &mut AmstradGateArray) -> (usize, usize) {
        let mut fb = framebuffer();
        let mut rise = None;
        let mut width = 0;
        for i in 1..=FRAME_TICKS {
            Ula::tick(g, &UNCONTENDED, 0, false, false, false, &mut fb);
            if g.interrupt_active() {
                rise.get_or_insert(i);
                width += 1;
            }
        }
        (rise.unwrap_or(0), width)
    }

    #[test]
    fn default_is_the_same_as_new() {
        let g = AmstradGateArray::default();
        assert_eq!(g.border_color(), 7);
        assert!(g.cpu_clock_active());
        assert!(!g.interrupt_active());
        assert_eq!(g.frame_timing().tstates_per_frame, 70_908);
    }

    #[test]
    fn frame_is_two_ticks_per_t_state() {
        let g = AmstradGateArray::new();
        assert_eq!(FRAME_TICKS, g.frame_timing().tstates_per_frame as usize * 2);
    }

    #[test]
    fn interrupt_pulse_is_32_t_states_on_line_248() {
        let mut g = AmstradGateArray::new();
        let (rise, width) = interrupt_rise_and_width(&mut g);
        assert_eq!(rise, INT_RISE_TICK);
        // 64 half-cycle ticks is 32 T-states, the ASIC's shorter pulse.
        assert_eq!(width, 64);
    }

    #[test]
    fn reattach_config_restores_plus2a_timing() {
        let mut g = AmstradGateArray::new();
        // What a snapshot restore leaves behind: the engine's skipped
        // config falls back to the 48K one.
        g.engine.set_config(&ula_engine::CONFIG_48K);
        g.reattach_config();
        let (rise, width) = interrupt_rise_and_width(&mut g);
        assert_eq!(rise, INT_RISE_TICK);
        assert_eq!(width, 64);
    }

    #[test]
    fn end_frame_rewinds_the_beam_and_counts_flash() {
        let mut g = AmstradGateArray::new();
        let mut fb = framebuffer();
        for _ in 0..1_000 {
            Ula::tick(&mut g, &UNCONTENDED, 0, false, false, false, &mut fb);
        }
        assert_ne!((g.engine.scan, g.engine.pixel), (0, 0));
        g.end_frame();
        assert_eq!((g.engine.scan, g.engine.pixel), (0, 0));
        assert_eq!(g.engine.flash_counter, 1);
        assert!(!g.engine.flash_active);

        // The beam restarts from the top: the interrupt is a full
        // frame-start distance away again.
        let (rise, _) = interrupt_rise_and_width(&mut g);
        assert_eq!(rise, INT_RISE_TICK);
    }

    #[test]
    fn flash_turns_on_after_sixteen_frames() {
        let mut g = AmstradGateArray::new();
        for _ in 0..15 {
            g.end_frame();
        }
        assert!(!g.engine.flash_active);
        g.end_frame();
        assert!(g.engine.flash_active);
    }

    #[test]
    fn contended_memory_access_stalls_only_in_the_display_window() {
        let mut g = AmstradGateArray::new();
        let mut fb = framebuffer();
        let mut stalls = 0;
        for _ in 0..FRAME_TICKS {
            let phase = (g.engine.pixel as usize) & 0x0F;
            let scan = g.engine.scan;
            Ula::tick(&mut g, &CONTENDED, 0x4000, true, false, false, &mut fb);
            if !g.cpu_clock_active() {
                stalls += 1;
                assert!(scan < 192, "stall outside the display lines at {scan}");
                assert!(DELAY_TABLE_PLUS2A[phase], "stall at phase {phase}");
            }
        }
        assert!(stalls > 0);
    }

    #[test]
    fn no_contention_without_mreq() {
        // Control: the same address and memory do stall once MREQ is up.
        assert!(stalls_over_a_frame(&CONTENDED, 0x4000, true, false) > 0);
        assert_eq!(stalls_over_a_frame(&CONTENDED, 0x4000, false, false), 0);
    }

    #[test]
    fn io_requests_are_never_contended() {
        // An even port in the contended range with IORQ asserted: the
        // Sinclair ULAs would stall here, the gate array does not.
        assert_eq!(stalls_over_a_frame(&CONTENDED, 0x40FE, false, true), 0);
    }

    #[test]
    fn uncontended_addresses_never_stall() {
        assert_eq!(stalls_over_a_frame(&UNCONTENDED, 0x8000, true, false), 0);
    }

    #[test]
    fn read_fe_clears_ear_with_no_keys_down() {
        let g = AmstradGateArray::new();
        assert_eq!(g.read_fe(0xFEFE, &[0x1F; 8]), 0xBF);
    }

    #[test]
    fn read_fe_keeps_ear_low_whatever_was_written() {
        let mut g = AmstradGateArray::new();
        // MIC and EAR bits both set: on a 48K this would raise bit 6.
        g.write_fe(0x18);
        assert_eq!(g.read_fe(0xFEFE, &[0x1F; 8]) & 0x40, 0);
        assert_eq!(g.border_color(), 0);
    }

    #[test]
    fn read_fe_reports_keys_on_selected_half_rows() {
        let g = AmstradGateArray::new();
        let mut keys = [0x1F; 8];
        keys[0] = 0x1E; // bit 0 down on half-row 0
        keys[7] = 0x0F; // bit 4 down on half-row 7

        // $FEFE selects only half-row 0.
        assert_eq!(g.read_fe(0xFEFE, &keys), 0xBE);
        // $7FFE selects only half-row 7.
        assert_eq!(g.read_fe(0x7FFE, &keys), 0xAF);
        // $00FE selects every half-row, so both keys show.
        assert_eq!(g.read_fe(0x00FE, &keys), 0xAE);
        // $FFFE selects none.
        assert_eq!(g.read_fe(0xFFFE, &keys), 0xBF);
    }
}
