# i3-window-killer: an i3wm node killer

## Description

This simple program prompts the user for confirmation, using **rofi**, before killing the focused node tree on **i3wm**.

The prompt specifies the titles (and classes) of the node(s) composing the targeted tree to quickly assess that the intended nodes are about to be killed.

### Screenshots

Single node prompt exceeding the maximum prompt length:

![single node prompt capture](screen2.jpg)

Multiple nodes prompt:

![multiple nodes prompt capture](screen1.jpg)

## Requirements

- [i3](https://github.com/i3/i3) window manager
- [rofi](https://github.com/davatorium/rofi) (merely used as `dmenu` here)

## Build

The program is built using `cargo` (_comes with [rustup](https://www.rust-lang.org/tools/install)_).

To build, run `cargo b --release`. The binary will be under `target/release`.

## Usage

Use the binary in your i3 config as follows:

```
bindsym $mod+Shift+a exec --no-startup-id path/to/i3-window-killer
```

## Customize

The program can easily be hacked to fit your preferences. Here are the key points to consider:

- change the prompt max/min length, separator string, ellipsis string ([here](src/lib.rs#L79-L85))
- change the prompt options ([here](src/lib.rs#L23))
- use a different prompt than `rofi` altogether
