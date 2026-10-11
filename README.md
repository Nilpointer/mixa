# mixa

A modern mixer for the Linux sound system ALSA, written in Rust with
[egui](https://github.com/emilk/egui). It is a reimagining of
[QasTools](https://gitlab.com/sebholt/qastools) by Sebastian Holtermann. mixa
merges QasTools' three apps into one window with a new visual design.

| QasTools  | mixa page         | What it does                                                    |
|-----------|-------------------|-----------------------------------------------------------------|
| QasMixer  | **Mixer**         | Playback/capture faders, mute & capture switches, source selectors |
| QasHctl   | **Controls**      | Every raw ALSA control element (bool/int/enum/bytes) with an editor |
| QasConfig | **Configuration** | Searchable browser for the ALSA configuration tree              |
| —         | **Device**        | Card information and stats                                      |

## Features

- Custom faders with dB scale marks, smooth animation and gradient fills. You
  can drag a fader, scroll on it, or double-click it to reset to 0 dB.
- Perceptual volume curve (same mapping as `alsamixer`), with linear raw mode
  available as an option.
- Stereo link/split per control. Right-click a control to hide it, split it,
  or set it to 0 dB, maximum or minimum.
- Toggle switches and segmented controls for switches and enumerations.
- Dark and light themes with five accent gradients. Settings persist between
  runs.
- Zero idle CPU: a watcher thread wakes the UI only on ALSA control events.
- Hot-plug aware: card list refresh, automatic fallback when a card disappears.
- Keyboard: `Ctrl+1…4` pages, `F5` reload, `Ctrl+,` settings, `Ctrl+Q` quit.

## Build

Requires the ALSA development files (`alsa-lib-devel` / `libasound2-dev`).

```sh
cargo build --release
./target/release/mixa            # default device
./target/release/mixa -D hw:1    # specific device
./target/release/mixa -c 0       # card by index
```

The UI uses the Inter font when it's installed and falls back to egui's
bundled font otherwise.

## Layout

```
src/snd/      alsa-lib wrappers (simple mixer, hctl, config tree, cards, event watcher)
src/ui/       theme, custom widgets, pages, app shell
```

For development, `MIXA_SCREENSHOT=out.png [MIXA_PAGE=0..3] mixa` renders a
frame, saves it as a PNG and exits.

## License

MIT — see [LICENSE](LICENSE). mixa is inspired by QasTools, which is also
MIT-licensed (© QasTools contributors).
