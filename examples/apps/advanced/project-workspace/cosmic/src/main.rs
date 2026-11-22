use cosmic::iced::Length;
use cosmic::prelude::*;
use cosmic::{Core, executor, widget};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy)]
enum Page {
    Overview,
    Projects,
    Settings,
}
#[derive(Debug, Clone)]
enum Message {
    Page(Page),
    Name(String),
    Search(String),
    Remove(usize),
    CycleStatus(usize),
    Progress(usize, f32),
    ToggleCompact,
    Filter(String),
    Edit(Option<usize>),
    DraftStatus(String),
    DraftProgress(f32),
    Save,
    Cancel,
    Theme,
}
#[derive(Clone, Serialize, Deserialize)]
struct Project {
    name: String,
    progress: f32,
    status: String,
}
struct App {
    core: Core,
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
    draft_progress: f32,
    error: Option<String>,
}
impl cosmic::Application for App {
    type Executor = executor::Default;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = "org.rupicola.ProjectWorkspace";
    fn core(&self) -> &Core {
        &self.core
    }
    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }
    fn init(core: Core, _: ()) -> (Self, cosmic::app::Task<Message>) {
        let mut app = Self {
            core,
            page: Page::Overview,
            projects: vec![
                Project {
                    name: "Website refresh".into(),
                    progress: 0.65,
                    status: "In progress".into(),
                },
                Project {
                    name: "Customer interviews".into(),
                    progress: 0.1,
                    status: "Planned".into(),
                },
                Project {
                    name: "Release checklist".into(),
                    progress: 1.0,
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
            draft_progress: 0.0,
            error: None,
        };
        app.load();
        let theme = if app.dark {
            cosmic::theme::Theme::dark()
        } else {
            cosmic::theme::Theme::light()
        };
        (app, cosmic::command::set_theme(theme))
    }
    fn update(&mut self, message: Message) -> cosmic::app::Task<Message> {
        match message {
            Message::Page(page) => self.page = page,
            Message::Name(v) => self.input = v,
            Message::Search(v) => self.search = v,
            Message::CycleStatus(i) => {
                if let Some(p) = self.projects.get_mut(i) {
                    p.status = match p.status.as_str() {
                        "Planned" => "In progress",
                        "In progress" => "Completed",
                        _ => "Planned",
                    }
                    .into();
                }
            }
            Message::Remove(i) => {
                if i < self.projects.len() {
                    self.projects.remove(i);
                }
            }
            Message::Progress(i, v) => {
                if let Some(p) = self.projects.get_mut(i) {
                    p.progress = v;
                }
            }
            Message::ToggleCompact => self.compact = !self.compact,
            Message::Filter(v) => self.filter = v,
            Message::Theme => {
                self.dark = !self.dark;
                if !self.blocked {
                    self.error = self.persist().err();
                }
                return cosmic::command::set_theme(if self.dark {
                    cosmic::theme::Theme::dark()
                } else {
                    cosmic::theme::Theme::light()
                });
            }
            Message::DraftStatus(v) => self.draft_status = v,
            Message::DraftProgress(v) => self.draft_progress = v,
            Message::Edit(index) => {
                self.editing = Some(index);
                if let Some(p) = index.and_then(|i| self.projects.get(i)) {
                    self.input = p.name.clone();
                    self.draft_status = p.status.clone();
                    self.draft_progress = p.progress;
                } else {
                    self.input.clear();
                    self.draft_status = "Planned".into();
                    self.draft_progress = 0.0;
                }
            }
            Message::Cancel => self.editing = None,
            Message::Save => {
                let name = self.input.trim();
                if name.is_empty() {
                    self.error = Some("Enter a project name.".into());
                    return cosmic::Task::none();
                }
                let p = Project {
                    name: name.into(),
                    status: self.draft_status.clone(),
                    progress: self.draft_progress,
                };
                if let Some(Some(i)) = self.editing {
                    if let Some(existing) = self.projects.get_mut(i) {
                        *existing = p;
                    }
                } else {
                    self.projects.push(p);
                }
                self.editing = None;
            }
        }
        if !self.blocked {
            self.error = self.persist().err();
        }
        cosmic::Task::none()
    }
    fn view(&self) -> Element<'_, Message> {
        let nav = cosmic::iced::widget::row![
            widget::button::standard("Overview").on_press(Message::Page(Page::Overview)),
            widget::button::standard("Projects").on_press(Message::Page(Page::Projects)),
            widget::button::standard("Settings").on_press(Message::Page(Page::Settings))
        ]
        .spacing(10);
        let mut content = widget::column::with_capacity(10)
            .spacing(12)
            .push(widget::text::body(self.error.as_deref().unwrap_or("")));
        match self.page {
            Page::Overview => {
                let metric = |label: &'static str, value: usize| {
                    widget::container(
                        cosmic::iced::widget::column![
                            widget::text::body(label),
                            widget::text::title1(value.to_string())
                        ]
                        .spacing(8),
                    )
                    .padding(20)
                    .width(Length::Fill)
                    .class(cosmic::theme::Container::Card)
                };
                content = content
                    .push(widget::text::caption("AT A GLANCE"))
                    .push(widget::text::title2("Overview"))
                    .push(
                        cosmic::iced::widget::row![
                            metric("Projects", self.projects.len()),
                            metric(
                                "In progress",
                                self.projects
                                    .iter()
                                    .filter(|p| p.status == "In progress")
                                    .count()
                            ),
                            metric(
                                "Completed",
                                self.projects
                                    .iter()
                                    .filter(|p| p.status == "Completed")
                                    .count()
                            )
                        ]
                        .spacing(2),
                    )
                    .push(widget::text::title3("Recent projects"));
                for p in self.projects.iter().rev().take(5) {
                    content =
                        content.push(widget::text::body(format!("{} — {}", p.name, p.status)));
                }
            }
            Page::Projects => {
                content = content
                    .push(widget::text::caption("WORK QUEUE"))
                    .push(widget::text::title2("Projects"))
                    .push(widget::button::suggested("Add project").on_press(Message::Edit(None)))
                    .push(
                        cosmic::iced::widget::row![
                            widget::text_input::search_input("Search projects", &self.search)
                                .on_input(Message::Search),
                            cosmic::iced::widget::pick_list(
                                ["All statuses", "Planned", "In progress", "Completed"]
                                    .map(str::to_string),
                                Some(self.filter.clone()),
                                Message::Filter
                            )
                        ]
                        .spacing(10),
                    );
                for (i, p) in self.projects.iter().enumerate().filter(|(_, p)| {
                    p.name.to_lowercase().contains(&self.search.to_lowercase())
                        && (self.filter == "All statuses" || p.status == self.filter)
                }) {
                    content = content.push(
                        cosmic::iced::widget::column![
                            cosmic::iced::widget::row![
                                widget::text::body(&p.name).width(Length::Fill),
                                widget::button::standard(&p.status)
                                    .on_press(Message::CycleStatus(i)),
                                widget::button::standard("Edit").on_press(Message::Edit(Some(i))),
                                widget::button::standard("Delete").on_press(Message::Remove(i))
                            ]
                            .spacing(10),
                            cosmic::iced::widget::row![
                                widget::slider(0.0..=1.0, p.progress, move |v| Message::Progress(
                                    i, v
                                ))
                                .step(0.05_f32),
                                widget::text::body(format!("{:.0}%", p.progress * 100.0))
                            ]
                            .spacing(10),
                            widget::progress_bar::linear::Linear::new().progress(p.progress)
                        ]
                        .spacing(if self.compact { 3 } else { 8 }),
                    );
                }
            }
            Page::Settings => {
                content = content
                    .push(widget::text::caption("PREFERENCES"))
                    .push(widget::text::title2("Settings"))
                    .push(
                        widget::button::standard(if self.compact {
                            "Use spacious list"
                        } else {
                            "Use compact list"
                        })
                        .on_press(Message::ToggleCompact),
                    )
                    .push(widget::text::body(
                        "Projects and settings are stored locally.",
                    ));
            }
        }
        let header = widget::container(
            cosmic::iced::widget::column![
                widget::text::caption("FIELDWORK / PROJECTS"),
                cosmic::iced::widget::row![
                    widget::text::title1("Project workspace").width(Length::Fill),
                    widget::button::standard(if self.dark {
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
        .class(cosmic::theme::Container::Card);
        let tabs = widget::container(nav).padding([10, 32]).width(Length::Fill);
        let body =
            widget::container(cosmic::iced::widget::scrollable(content).height(Length::Fill))
                .padding([32, 32])
                .width(Length::Fill)
                .height(Length::Fill);
        let main = cosmic::iced::widget::column![header, tabs, body]
            .width(Length::Fill)
            .height(Length::Fill);
        if self.editing.is_some() {
            let editor = cosmic::iced::widget::column![
                widget::text::title2(if self.editing == Some(None) {
                    "Add project"
                } else {
                    "Edit project"
                }),
                widget::text::body("Project name"),
                widget::text_input::text_input("Project name", &self.input).on_input(Message::Name),
                widget::text::body("Status"),
                cosmic::iced::widget::pick_list(
                    ["Planned", "In progress", "Completed"].map(str::to_string),
                    Some(self.draft_status.clone()),
                    Message::DraftStatus
                ),
                widget::text::body("Progress"),
                widget::slider(0.0..=1.0, self.draft_progress, Message::DraftProgress)
                    .step(0.05_f32),
                widget::text::body(format!("{:.0}%", self.draft_progress * 100.0)),
                widget::text::body(self.error.as_deref().unwrap_or("")),
                cosmic::iced::widget::row![
                    widget::button::standard("Cancel").on_press(Message::Cancel),
                    widget::button::suggested("Save project").on_press(Message::Save)
                ]
                .spacing(10)
            ]
            .spacing(12)
            .padding(24);
            widget::container(
                widget::container(editor)
                    .width(480)
                    .class(cosmic::theme::Container::Card),
            )
            .center(Length::Fill)
            .into()
        } else {
            main.into()
        }
    }
}
fn main() -> cosmic::iced::Result {
    cosmic::app::run::<App>(cosmic::app::Settings::default(), ())
}

#[derive(Serialize, Deserialize)]
struct Saved {
    projects: Vec<Project>,
    compact: bool,
    dark: bool,
}
impl App {
    fn path() -> Result<std::path::PathBuf, String> {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share"))
            })
            .ok_or("No data directory")?;
        Ok(base.join("rupicola-examples/cosmic-workspace.json"))
    }
    fn persist(&self) -> Result<(), String> {
        let path = Self::path()?;
        std::fs::create_dir_all(path.parent().ok_or("No data directory")?)
            .map_err(|e| e.to_string())?;
        let tmp = path.with_extension("tmp");
        let bytes = serde_json::to_vec(&Saved {
            projects: self.projects.clone(),
            compact: self.compact,
            dark: self.dark,
        })
        .map_err(|e| e.to_string())?;
        std::fs::write(&tmp, bytes)
            .and_then(|()| std::fs::rename(&tmp, &path))
            .map_err(|e| format!("Could not save: {e}"))
    }
    fn load(&mut self) {
        let result = Self::path().and_then(|p| match std::fs::read(p) {
            Ok(bytes) => serde_json::from_slice::<Saved>(&bytes)
                .map(Some)
                .map_err(|e| e.to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        });
        match result {
            Ok(Some(saved)) => {
                self.projects = saved.projects;
                self.compact = saved.compact;
                self.dark = saved.dark;
            }
            Ok(None) => {}
            Err(e) => {
                self.blocked = true;
                self.error = Some(format!("Could not load saved data; saving disabled: {e}"));
            }
        }
    }
}
