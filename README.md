# term-render

A basic ASCII-based terminal renderer.

## Installation

term-render may be installed through one of the following methods:

### Latest release

You can download the latest release of term-render
through [this repository's latest releases](https://github.com/Jaxydog/term-render/releases).

### Cargo installation

You may alternatively install term-render directly through Cargo.

```sh
cargo install --locked --git https://github.com/Jaxydog/term-render.git
```

### Compile from source

term-render may also be compiled directly using Git and Cargo.

```sh
git clone https://github.com/Jaxydog/term-render.git
cd term-render
cargo build --release
```

The compiled binary will be located at
`./target/release/term-render` (Unix) or
`.\target\release\term-render.exe` (Windows).

## Usage

term-render currently is very simple,
only providing a small selection of arguments.

```
Usage: term-render [OPTIONS] <PATH>

Arguments:
  <PATH>  The path to an image

Options:
  -f, --font <FONT>  Specifies the font used by the terminal during rendering for more accurate character brightnesses
  -c, --clean        Whether to clean up all caches before running
  -p, --plain        Whether to draw the image without color
  -h, --help         Print help
```

It's recommended (but not required) to set the `--font` argument
to your terminal's configured font so that the rendered image uses
more accurate character brightness values.

The first invocation with each font may be slightly slower
than expected due to the program needing to calculate the
brightness of each rendered character.
Subsequent runs should be much faster due to caching.

## License

term-render is free software:
you can redistribute it and/or modify it under the terms
of the GNU General Public License as published by the
Free Software Foundation,
either version 3 of the License,
or (at your option) any later version.

term-render is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY;
without even the implied warranty of MERCHANTABILITY
or FITNESS FOR A PARTICULAR PURPOSE.
See the GNU General Public License for more details.

You should have received a copy
of the GNU General Public License along with term-render.
If not, see https://www.gnu.org/licenses/.
