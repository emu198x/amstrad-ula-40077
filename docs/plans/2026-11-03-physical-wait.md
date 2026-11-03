# Drive the 40077's physical WAIT output

Approved with Amstrad native snapshot v20 for emu198x/emu198x#2265 on
2026-11-03. Amstrad's +3 manual PDF p275 / printed p273 specifies WAIT;
the 40077 service schematic connects NWAIT separately from the CPU clock.
The prior clock-gating model also stopped M1 refresh in all-RAM mode.

1. In `crates/emu198x-amstrad-ula-40077/src/lib.rs`, serialize an explicit
   WAIT output, expose `wait_active`, and keep the CPU clock continuous.
   Use the gate-array WAIT waveform, not the Sinclair clock-hold mask.
   Verify falling-edge samples give [1,0,7,6,5,4,3,2] extra T-states.
2. Preserve rendering/IRQ/port tests; add continuous clock during WAIT,
   MREQ-only qualification and refresh/IO controls. A deliberate false WAIT
   output must make the new behavioral tests fail. No dependency is added.
3. Document migration: consumers feed Z80 WAIT at each edge, never suppress
   CPU clocks for this part. The extra live output changes the save layout;
   consumers must version the enclosing snapshot and reject old payloads.
4. Run fmt, all-target Clippy and workspace tests. Publish a breaking 0.x
   minor through normal CI/release before the umbrella consumes it.
5. Consumer qualification belongs to the umbrella plan, including corrected
   shared Z80 WAIT edges, v20 rejection of Amstrad v19, native mid-WAIT restore,
   original firmware Cybernoid boot and all 103 retained catalogue entries.

References are provenance paths in the private primary library:
`reference/by-system/sinclair-zx-spectrum/zx-spectrum-plus-3-manual-amstrad.*`,
`zx-spectrum-plus3-service-manual.*`, `ula-timing-expanded.md`; Zilog UM0080
figures 5-7 and 9 in `reference/by-topic/cpu-z80/`. Vendored SpecIde
`ULA::generateVideoDataGa`/`delayGa` supplies independent signal-level precedent.
