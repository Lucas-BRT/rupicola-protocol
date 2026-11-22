mod common;

use rupicola_protocol::ToolkitKind;

fn main() {
    common::run_example_cli(
        "hello-gtk4",
        ToolkitKind::Gtk4,
        "Hello GTK4",
        Some("gtk4::Label"),
    );
}
