mod common;

use rupicola_protocol::ToolkitKind;

fn main() {
    common::run_example_cli(
        "hello-cosmic",
        ToolkitKind::Cosmic,
        "Hello COSMIC",
        Some("cosmic::widget::text"),
    );
}
