use iced::widget::{
    button, checkbox, column, container, pick_list, progress_bar, row, scrollable, slider, text,
    text_input,
};
use iced::{Element, Length};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
enum Message {
    Page(Page),
    Name(String),
    Remove(usize),
    CycleStatus(usize),
    Search(String),
    Progress(usize, u8),
    Compact(bool),
    Filter(String),
    Edit(Option<usize>),
    DraftStatus(String),
    DraftProgress(u8),
    Save,
    Cancel,
    Theme,
}
#[derive(Debug, Clone, Copy, PartialEq)]
enum Page {
    Overview,
    Projects,
    Settings,
}
#[derive(Clone, Serialize, Deserialize)]
struct Project {
    name: String,
    progress: u8,
    status: String,
}
struct Workspace {
    page: Page,
    projects: Vec<Project>,
    input: String,
    search: String,
    compact: bool,
    dark: bool,
    blocked: bool,
    filter: String,
    editing: Option<Option<usize>>,
    draft_status: String,
    draft_progress: u8,
    error: Option<String>,
}
impl Default for Workspace {
    fn default() -> Self {
        Self {
            page: Page::Overview,
            projects: vec![
                Project {
                    name: "Website refresh".into(),
                    progress: 65,
                    status: "In progress".into(),
                },
                Project {
                    name: "Customer interviews".into(),
                    progress: 10,
                    status: "Planned".into(),
                },
                Project {
                    name: "Release checklist".into(),
                    progress: 100,
                    status: "Completed".into(),
                },
            ],
            input: String::new(),
            search: String::new(),
            compact: false,
            dark: false,
            blocked: false,
            filter: "All statuses".into(),
            editing: None,
            draft_status: "Planned".into(),
            draft_progress: 0,
            error: None,
        }
    }
}
fn update(state: &mut Workspace, msg: Message) {
    match msg {
        Message::Page(p) => state.page = p,
        Message::Name(v) => state.input = v,
        Message::Search(v) => state.search = v,
        Message::CycleStatus(i) => {
            if let Some(p) = state.projects.get_mut(i) {
                p.status = match p.status.as_str() {
                    "Planned" => "In progress",
                    "In progress" => "Completed",
                    _ => "Planned",
                }
                .into();
            }
        }
        Message::Remove(i) => {
            if i < state.projects.len() {
                state.projects.remove(i);
            }
        }
        Message::Progress(i, v) => {
            if let Some(p) = state.projects.get_mut(i) {
                p.progress = v;
            }
        }
        Message::Compact(v) => state.compact = v,
        Message::Filter(v) => state.filter = v,
        Message::Theme => state.dark = !state.dark,
        Message::DraftStatus(v) => state.draft_status = v,
        Message::DraftProgress(v) => state.draft_progress = v,
        Message::Edit(index) => {
            state.editing = Some(index);
            if let Some(p) = index.and_then(|i| state.projects.get(i)) {
                state.input = p.name.clone();
                state.draft_status = p.status.clone();
                state.draft_progress = p.progress;
            } else {
                state.input.clear();
                state.draft_status = "Planned".into();
                state.draft_progress = 0;
            }
        }
        Message::Cancel => state.editing = None,
        Message::Save => {
            let name = state.input.trim();
            if name.is_empty() {
                state.error = Some("Enter a project name.".into());
                return;
            }
            let p = Project {
                name: name.into(),
                status: state.draft_status.clone(),
                progress: state.draft_progress,
            };
            if let Some(Some(i)) = state.editing {
                if let Some(existing) = state.projects.get_mut(i) {
                    *existing = p;
                }
            } else {
                state.projects.push(p);
            }
            state.editing = None;
        }
    }
    if !state.blocked {
        state.error = persist(state).err();
    }
}
fn view(state: &Workspace) -> Element<'_, Message> {
    let nav = row![
        button("Overview").on_press(Message::Page(Page::Overview)),
        button("Projects").on_press(Message::Page(Page::Projects)),
        button("Settings").on_press(Message::Page(Page::Settings))
    ]
    .spacing(10);
    let content = match state.page {
        Page::Overview => {
            let metric = |label: &'static str, value: usize| {
                container(column![text(label).size(16), text(value).size(32)].spacing(8))
                    .padding(20)
                    .width(Length::Fill)
                    .style(container::rounded_box)
            };
            column![
                text("AT A GLANCE").size(12),
                text("Overview").size(30),
                row![
                    metric("Projects", state.projects.len()),
                    metric(
                        "In progress",
                        state
                            .projects
                            .iter()
                            .filter(|p| p.status == "In progress")
                            .count()
                    ),
                    metric(
                        "Completed",
                        state
                            .projects
                            .iter()
                            .filter(|p| p.status == "Completed")
                            .count()
                    )
                ]
                .spacing(2),
                text("Recent projects").size(22),
                column(
                    state.projects.iter().rev().take(5).map(|p| text(format!(
                        "{}  —  {}",
                        p.name, p.status
                    ))
                    .into())
                )
                .spacing(12)
            ]
            .spacing(16)
        }
        Page::Projects => {
            let mut list = column![
                text("WORK QUEUE").size(12),
                text("Projects").size(30),
                button("Add project").on_press(Message::Edit(None)),
                row![
                    text_input("Search projects", &state.search).on_input(Message::Search),
                    pick_list(
                        ["All statuses", "Planned", "In progress", "Completed"].map(str::to_string),
                        Some(state.filter.clone()),
                        Message::Filter
                    )
                ]
                .spacing(12)
            ]
            .spacing(12);
            for (i, p) in state.projects.iter().enumerate().filter(|(_, p)| {
                p.name.to_lowercase().contains(&state.search.to_lowercase())
                    && (state.filter == "All statuses" || p.status == state.filter)
            }) {
                list = list.push(
                    column![
                        row![
                            text(&p.name).width(Length::Fill),
                            button(p.status.as_str()).on_press(Message::CycleStatus(i)),
                            button("Edit").on_press(Message::Edit(Some(i))),
                            button("Delete").on_press(Message::Remove(i))
                        ]
                        .spacing(12),
                        row![
                            slider(0..=100, p.progress, move |v| Message::Progress(i, v)),
                            text(format!("{}%", p.progress))
                        ]
                        .spacing(12),
                        progress_bar(0.0..=100.0, f32::from(p.progress))
                    ]
                    .spacing(if state.compact { 3 } else { 8 }),
                );
            }
            list
        }
        Page::Settings => column![
            text("PREFERENCES").size(12),
            text("Settings").size(30),
            checkbox(state.compact)
                .label("Compact project list")
                .on_toggle(Message::Compact),
            text("Projects and settings are stored locally.")
        ]
        .spacing(14),
    };
    let header = container(
        column![
            text("FIELDWORK / PROJECTS").size(12),
            row![
                text("Project workspace").size(36).width(Length::Fill),
                button(if state.dark {
                    "Light theme"
                } else {
                    "Dark theme"
                })
                .on_press(Message::Theme)
            ]
        ]
        .spacing(8),
    )
    .padding([24, 32])
    .width(Length::Fill)
    .style(|_| {
        container::Style::default()
            .background(iced::Color::from_rgb8(23, 61, 64))
            .color(iced::Color::WHITE)
    });
    let tabs = container(nav)
        .padding([10, 32])
        .width(Length::Fill)
        .style(container::secondary);
    let body = container(
        scrollable(column![text(state.error.as_deref().unwrap_or("")), content])
            .height(Length::Fill),
    )
    .padding([32, 32])
    .width(Length::Fill)
    .height(Length::Fill);
    let main = column![header, tabs, body]
        .width(Length::Fill)
        .height(Length::Fill);
    if state.editing.is_some() {
        let editor = column![
            text(if state.editing == Some(None) {
                "Add project"
            } else {
                "Edit project"
            })
            .size(28),
            text("Project name"),
            text_input("Project name", &state.input).on_input(Message::Name),
            text("Status"),
            pick_list(
                ["Planned", "In progress", "Completed"].map(str::to_string),
                Some(state.draft_status.clone()),
                Message::DraftStatus
            ),
            text("Progress"),
            slider(0..=100, state.draft_progress, Message::DraftProgress),
            text(format!("{}%", state.draft_progress)),
            text(state.error.as_deref().unwrap_or("")),
            row![
                button("Cancel").on_press(Message::Cancel),
                button("Save project").on_press(Message::Save)
            ]
            .spacing(10)
        ]
        .spacing(12)
        .padding(24);
        container(container(editor).width(480).style(container::rounded_box))
            .center(Length::Fill)
            .into()
    } else {
        main.into()
    }
}
fn main() -> iced::Result {
    iced::application(load, update, view)
        .theme(|s: &Workspace| {
            if s.dark {
                iced::Theme::Dark
            } else {
                iced::Theme::Light
            }
        })
        .run()
}

#[derive(Serialize, Deserialize)]
struct Saved {
    projects: Vec<Project>,
    compact: bool,
    dark: bool,
}
fn path() -> Result<std::path::PathBuf, String> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share"))
        })
        .ok_or("No data directory")?;
    Ok(base.join("rupicola-examples/iced-workspace.json"))
}
fn persist(s: &Workspace) -> Result<(), String> {
    let path = path()?;
    let dir = path.parent().ok_or("No data directory")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("tmp");
    let bytes = serde_json::to_vec(&Saved {
        projects: s.projects.clone(),
        compact: s.compact,
        dark: s.dark,
    })
    .map_err(|e| e.to_string())?;
    std::fs::write(&tmp, bytes)
        .and_then(|()| std::fs::rename(&tmp, &path))
        .map_err(|e| format!("Could not save: {e}"))
}
fn load() -> Workspace {
    let mut state = Workspace::default();
    let result = path().and_then(|p| match std::fs::read(p) {
        Ok(bytes) => serde_json::from_slice::<Saved>(&bytes)
            .map(Some)
            .map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    });
    match result {
        Ok(Some(saved)) => {
            state.projects = saved.projects;
            state.compact = saved.compact;
            state.dark = saved.dark;
        }
        Ok(None) => {}
        Err(e) => {
            state.blocked = true;
            state.error = Some(format!("Could not load saved data; saving disabled: {e}"));
        }
    }
    state
}
