# GTK4 counter

A native GTK4 counter with increment and reset controls. No protocol instrumentation is embedded.

## Dependencies and execution

Rust 1.99.1, Cargo, GTK4 and its GLib/GIO libraries, and a graphical session are required. Development linker files for GTK4 must be installed (for example, `libgtk-4-dev` on Debian).

Run `cargo run` in this directory. X11 and Wayland support depend on the local GTK4 installation.
