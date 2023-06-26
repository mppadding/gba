# GBA Emulator
Multi-threaded GBA emulator with debugger support.

## Build
Build project using cargo:
```
cargo run
```

To enable debugger support set the `debugger` feature:
```
cargo run --features debugger
```

To enable CPU backtrace support, set the `backtrace` or `full-backtrace` feature.
`full-backtrace` resolves register and memory values, `backtrace` only shows `R1`.
```
cargo run --features backtrace
```

## TODO
### Rendering
- [ ] Windowed mode
- [ ] Alpha blending
- [ ] Affine sprites
- [ ] Mosaic

### Hardware
- [ ] IRQ testing
- [ ] DMA
- [ ] Serial
- [ ] Sound
- [ ] Timers

## ROMs
Some test roms are working and others still contain bugs.
### TONC
Working:
- [x] Key Demo
- [x] Pageflip
- [x] BM modes
- [x] M3 demo
- [x] brin demo
- [x] obj demo

Not working or bugged:
- [ ] sbb_reg => Draws sprites but doesnt use them (might be a bug in the demo itself)
- [ ] txt_bm => Text goofed up
- [ ] tte_demo => Requires SWI 15h
- [ ] irq_demo
- [ ] dma_demo => DMA3 with HBlank timing
- [ ] bld_demo => gfx_mode=01 for sprites
- [ ] big_map
- [ ] swi_vsync => obj_mode=11
