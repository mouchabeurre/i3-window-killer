# i3-window-killer: kill i3wm windows with rofi (and style)

## Description

This program presents the user with a customizable **rofi** confirmation prompt before killing the focused **i3wm** node.

The prompt can be styled via a template file, exposing useful variables such as the windows _X11 titles_, _X11 classes_ or _desktop icon_, and allowing block control with loops and conditionals ([template syntax documentation](https://docs.rs/tinytemplate/1.2.1/tinytemplate/syntax/index.html)). An example can be found in [template.rasi](template.rasi).

### Screenshots

Prompt when killing a single window (with a template placing the prompt over the focused node):

![single window prompt capture](capture1.jpg)

Multiple windows prompt:

![multiple windows prompt capture](capture2.jpg)

## Requirements

- [i3](https://github.com/i3/i3) window manager
- [rofi](https://github.com/davatorium/rofi)

## Build

The program is built using `cargo` (comes with [rustup](https://www.rust-lang.org/tools/install)).

To build, run `cargo b --release`. The binary will be under `target/release`.

## Usage

Options are documented under the `--help` flag.

Use the binary in your i3 config as follows:

```
bindsym $mod+Shift+a exec --no-startup-id path/to/i3-window-killer
```

## Customize

The program can be hacked to fit your preferences. Here are the key points to consider:

- change the prompt choices ([fn prompt_user](src/lib.rs#L22))
- change the rofi subcommand flags ([fn prompt_user](src/lib.rs#L23))
- change the prompt text ([fn get_prompt_and_styles](src/lib.rs#L382))
