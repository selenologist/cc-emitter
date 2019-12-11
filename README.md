#cc-emitter

This little Rust program emits CC messages to MIDI ports. Other programs like this exist, but I was bored.

It uses the `midir` library by @BoddInagg, so it should run on multiple platforms.

Run with `--help` for usage.

This program was created because I needed a method that could easily be called from a keybinding, to turn off the local keyboard connection of a synthesizer. This is typically done using MIDI CC#122.

So to turn off local control (on all ports),
`cc-emitter "122:0"`

and to turn it back on
`cc-emitter "122:127"`

(normally you would filter by port name to only affect a specific device)
