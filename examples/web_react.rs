mod common;

use rupicola_protocol::ToolkitKind;

fn main() {
    common::run_example_cli(
        "hello-react",
        ToolkitKind::WebDom,
        "Hello React",
        Some("data-testid=hello"),
    );
}
