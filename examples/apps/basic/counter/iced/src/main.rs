use iced::Element;
use iced::widget::{button, column, text};

#[derive(Debug, Clone)]
enum Message {
    Increment,
    Reset,
}

fn update(count: &mut u64, message: Message) {
    match message {
        Message::Increment => *count = count.saturating_add(1),
        Message::Reset => *count = 0,
    }
}

fn view(count: &u64) -> Element<'_, Message> {
    column![
        text("Counter").size(32),
        text(count).size(48),
        button("Increment").on_press(Message::Increment),
        button("Reset").on_press(Message::Reset)
    ]
    .spacing(12)
    .padding(24)
    .into()
}

fn main() -> iced::Result {
    iced::run(update, view)
}
