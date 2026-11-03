# amstrad-ula-40077

The Amstrad 40077 gate array, the ZX Spectrum +2A, +2B and +3's ULA.

| Crate | Registry |
|---|---|
| [`emu198x-amstrad-ula-40077`](crates/emu198x-amstrad-ula-40077) | [crates.io](https://crates.io/crates/emu198x-amstrad-ula-40077) |

Part of [Emu198x](https://github.com/emu198x/emu198x), a family of
cycle-accurate retro-computing emulators. Used by the +2A, +2B and +3.

Each crate is versioned on its own and bumps only when it changes. Releases go
through release-plz: merging its release pull request tags the crate and
publishes it.

Licensed GPL-2.0-or-later.

## Migrating the physical WAIT output

The CPU clock runs continuously. Feed `AmstradGateArray::wait_active()` into
its CPU's WAIT input before every CPU edge; `Ula::cpu_clock_active()` remains
true. The CPU must sample WAIT on the documented falling edges and ignore it
during refresh. Ferranti clock gating is a different contract.

The gate array now saves its live WAIT output. Its serialized representation
has changed: the enclosing machine must version its snapshots and reject the
previous layout before decoding. The Spectrum +2A/+2B/+3 runtime uses v20.

Primary evidence: `reference/by-system/sinclair-zx-spectrum/ula-timing-expanded.md`,
the Amstrad +3 manual pp. 199 and 273, and the +3 service circuit diagrams
(IC2 NWAIT pin 24 is separate from CLK). The one-in-eight slot phase is also
compared with the vendored SpecIde gate-array implementation; propagation
within a half-cycle is not qualified.
