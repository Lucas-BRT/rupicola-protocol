mod common;

use rupicola_protocol::ToolkitKind;

fn main() {
    common::run_example_cli(
        "hello-html",
        ToolkitKind::WebDom,
        "Hello HTML",
        Some("h1.hero"),
    );
}
