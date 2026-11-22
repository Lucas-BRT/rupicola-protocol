mod common;

use rupicola_protocol::ToolkitKind;

fn main() {
    common::run_example_cli(
        "hello-iced",
        ToolkitKind::Iced,
        "Hello Iced",
        Some("iced::widget::text"),
    );
}
