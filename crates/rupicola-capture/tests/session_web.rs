//! The web backend as a session, in both flavours.

mod support;

#[maybe_async_cfg::maybe(
    idents(flavour(sync = "sync", async = "asynchronous")),
    sync(feature = "sync", drop_attrs(async_test)),
    async(feature = "async", drop_attrs(sync_test))
)]
mod flavour {
    #[allow(unused_imports)]
    use core::prelude::v1::test as sync_test;
    #[allow(unused_imports)]
    use tokio::test as async_test;
    #[maybe_async_cfg::only_if(sync)]
    const TAG: &str = "sync";
    #[maybe_async_cfg::only_if(async)]
    const TAG: &str = "async";

    use rupicola_capture::{
        flavour::{WebBackend, WebDiscovery as _, run_plan},
        session::{
            ColorScheme, Environment, ErrorClass, Plan, Step, Target, UiTreeMode, WaitCondition,
        },
    };
    use rupicola_protocol::{SnapshotSource, UiAction, WidgetKind};
    use std::{fs, sync::atomic::Ordering};

    const VIEWPORT: [u32; 2] = [1280, 720];
    /// Length of the recorded walkthrough.
    const CLIP_MS: u64 = 600;
    /// What the CRUD example's counter says after adding one task to its three.
    const COUNTER_AFTER_ADD: &str = "3 remaining · 4 total";
    /// The shortest acceptable clip: most of the 600 ms walkthrough.
    const MIN_CLIP_SECONDS: f64 = 0.4;
    /// A port nothing listens on.
    const LOCALE: &str = "pt-BR";
    const TIMEOUT_MS: u64 = 5000;
    const ADD_BUTTON_ID: &str = "add-btn";
    const CLOSED_PORT_URL: &str = "http://127.0.0.1:1/";

    /// Duration of a video in seconds, when `ffprobe` is installed.
    fn clip_seconds(path: &std::path::Path) -> Option<f64> {
        let output = std::process::Command::new("ffprobe")
            .args(["-v", "error", "-show_entries", "format=duration"])
            .args(["-of", "default=noprint_wrappers=1:nokey=1"])
            .arg(path)
            .output()
            .ok()?;
        String::from_utf8_lossy(&output.stdout).trim().parse().ok()
    }

    fn plan(url: &str, steps: Vec<Step>) -> Plan {
        let mut environment = Environment::new(VIEWPORT);
        environment.warmup_ms = 0;
        environment.color_scheme = Some(ColorScheme::Dark);
        environment.ui_tree = UiTreeMode::Full;
        Plan {
            target: Target {
                command: url.to_owned(),
                args: Vec::new(),
            },
            environment,
            steps,
            allow_degraded: false,
        }
    }

    #[sync_test]
    #[async_test]
    async fn one_page_serves_actions_two_checkpoints_and_a_clip() {
        let found = WebBackend::discover("chromium").await;
        let Some(backend) = found else {
            return;
        };
        let (url, stop) = crate::support::serve_example();
        let out = crate::support::out_dir(&format!("web-{TAG}-session"));
        let steps = vec![
            Step::Checkpoint {
                id: "before".to_owned(),
                full_page: false,
            },
            Step::StartRecording,
            Step::Action {
                action: UiAction::TypeText {
                    target_id: "new-task".to_owned(),
                    text: "Captured by a session".to_owned(),
                },
            },
            Step::Action {
                action: UiAction::Click {
                    target_id: "add-btn".to_owned(),
                },
            },
            Step::Wait {
                condition: WaitCondition::Text {
                    text: "Captured by a session".to_owned(),
                },
            },
            Step::Wait {
                condition: WaitCondition::Delay { ms: CLIP_MS },
            },
            Step::Checkpoint {
                id: "after".to_owned(),
                full_page: false,
            },
            Step::StopRecording {
                id: "walk".to_owned(),
            },
        ];
        let bundle = run_plan(&backend, &plan(&url, steps), &out).await.unwrap();
        stop.store(true, Ordering::Relaxed);

        assert_eq!(bundle.checkpoints.len(), 2);
        assert!(bundle.checkpoints.iter().all(|c| c.png.is_file()));
        let after = bundle.checkpoints[1]
            .snapshot
            .as_ref()
            .expect("DOM snapshot");
        assert_eq!(
            after.find_by_id("counter").unwrap().label.as_deref(),
            Some(COUNTER_AFTER_ADD)
        );
        assert_eq!(after.source, SnapshotSource::Dom);
        assert_eq!(
            after.find_by_id(ADD_BUTTON_ID).unwrap().kind,
            WidgetKind::Button,
            "the page's own button is a button in the shared vocabulary"
        );
        assert_eq!(
            after.find_by_id("new-task").unwrap().kind,
            WidgetKind::TextBox
        );
        assert!(bundle.clips[0].mp4.is_file());
        assert!(
            clip_seconds(&bundle.clips[0].mp4).is_none_or(|seconds| seconds >= MIN_CLIP_SECONDS),
            "the clip is shorter than the walkthrough"
        );
        assert!(!out.join(".recording-frames").exists());
        crate::support::keep_json(&out, "tree-after.json", &bundle.checkpoints[1].snapshot);
        crate::support::finish(&out);
    }

    #[sync_test]
    #[async_test]
    async fn the_page_gives_a_tree_only_as_much_as_the_plan_asks() {
        let found = WebBackend::discover("chromium").await;
        let Some(backend) = found else {
            return;
        };
        let (url, stop) = crate::support::serve_example();
        let out = crate::support::out_dir(&format!("web-{TAG}-ui-tree-modes"));
        let checkpoint = || {
            vec![Step::Checkpoint {
                id: "home".to_owned(),
                full_page: false,
            }]
        };
        let tree = |mode| {
            let mut plan = plan(&url, checkpoint());
            plan.environment.ui_tree = mode;
            // One folder per mode, so keeping the captures keeps all three.
            let dir = out.join(format!("{mode:?}").to_lowercase());
            let _ = fs::remove_dir_all(&dir);
            let backend = &backend;
            async move {
                let snapshot = run_plan(backend, &plan, &dir)
                    .await
                    .unwrap()
                    .checkpoints
                    .remove(0)
                    .snapshot;
                crate::support::keep_json(&dir, "tree.json", &snapshot);
                snapshot
            }
        };
        assert_eq!(tree(UiTreeMode::Off).await, None);
        let full = tree(UiTreeMode::Full).await.unwrap();
        assert!(full.find_by_id("counter").unwrap().label.is_some());
        assert!(full.title.is_some());
        let structure = tree(UiTreeMode::Structure).await.unwrap();
        stop.store(true, Ordering::Relaxed);
        let counter = structure.find_by_id("counter").unwrap();
        assert_eq!(counter.label, None, "structure keeps no text");
        assert!(counter.bounds.is_valid(), "but it keeps the layout");
        assert!(
            (structure.scale_factor - 1.0).abs() < f64::EPSILON,
            "a default scale is one pixel per logical unit"
        );
        crate::support::finish(&out);
    }

    #[sync_test]
    #[async_test]
    async fn a_missing_element_is_a_target_error() {
        let found = WebBackend::discover("chromium").await;
        let Some(backend) = found else {
            return;
        };
        let (url, stop) = crate::support::serve_example();
        let out = std::env::temp_dir().join(format!("rupicola-capture-{TAG}-session-web-missing"));
        let steps = vec![Step::Wait {
            condition: WaitCondition::Visible {
                target_id: "no-such-element".to_owned(),
            },
        }];
        let error = run_plan(&backend, &plan(&url, steps), &out)
            .await
            .unwrap_err();
        stop.store(true, Ordering::Relaxed);
        assert_eq!(error.class, ErrorClass::Target);
        let _ = fs::remove_dir_all(&out);
    }

    #[sync_test]
    #[async_test]
    async fn an_unreachable_url_is_a_target_error() {
        let found = WebBackend::discover("chromium").await;
        let Some(backend) = found else {
            return;
        };
        let out = std::env::temp_dir().join(format!("rupicola-capture-{TAG}-session-web-closed"));
        let error = run_plan(&backend, &plan(CLOSED_PORT_URL, Vec::new()), &out)
            .await
            .unwrap_err();
        assert_eq!(error.class, ErrorClass::Target);
        let _ = fs::remove_dir_all(&out);
    }

    #[sync_test]
    #[async_test]
    async fn a_target_that_names_a_selector_engine_is_used_as_given() {
        let found = WebBackend::discover("chromium").await;
        let Some(backend) = found else {
            return;
        };
        let (url, stop) = crate::support::serve_example();
        let out = std::env::temp_dir().join(format!("rupicola-capture-{TAG}-session-web-css"));
        let _ = fs::remove_dir_all(&out);
        let mut environment = Environment::new(VIEWPORT);
        environment.warmup_ms = 0;
        environment.locale = Some(LOCALE.to_owned());
        environment.timeout_ms = Some(TIMEOUT_MS);
        let steps = vec![
            // A plain id would be read as an element id, so this only works as given.
            Step::Action {
                action: UiAction::Click {
                    target_id: format!("css=button#{ADD_BUTTON_ID}"),
                },
            },
            Step::Checkpoint {
                id: "after".to_owned(),
                full_page: false,
            },
        ];
        let mut plan = plan(&url, steps);
        plan.environment = environment;
        let bundle = run_plan(&backend, &plan, &out).await.unwrap();
        stop.store(true, Ordering::Relaxed);
        assert!(bundle.checkpoints[0].png.is_file());
        let _ = fs::remove_dir_all(&out);
    }

    /// A page three screens tall, with an element near its end.
    const TALL_PAGE: &str = "data:text/html,<body style='margin:0;height:2160px;\
        background:linear-gradient(red,blue)'><div id='deep' style='position:absolute;\
        top:1800px;height:100px;width:100%;background:white'></div></body>";
    const TALL_PAGE_HEIGHT: u32 = 2160;
    const PNG_HEIGHT_OFFSET: usize = 20;
    const PNG_FIELD_BYTES: usize = 4;
    const DEEP_ID: &str = "deep";
    /// A pause longer than any scroll of the tall page needs.
    const LONG_PAUSE_MS: u64 = 600;

    fn checkpoint(id: &str, full_page: bool) -> Step {
        Step::Checkpoint {
            id: id.to_owned(),
            full_page,
        }
    }

    fn act(action: UiAction) -> Step {
        Step::Action { action }
    }

    fn press(key: &str) -> Step {
        act(UiAction::PressKey {
            key: key.to_owned(),
            modifiers: Vec::new(),
        })
    }

    /// Height in pixels of a PNG, read from its header.
    fn png_height(path: &std::path::Path) -> u32 {
        let bytes = fs::read(path).unwrap();
        let field = &bytes[PNG_HEIGHT_OFFSET..PNG_HEIGHT_OFFSET + PNG_FIELD_BYTES];
        u32::from_be_bytes(field.try_into().unwrap())
    }

    /// The bytes of each checkpoint a plan on the tall page takes, in order.
    async fn shoot_tall_page(
        backend: &WebBackend,
        name: &str,
        steps: Vec<Step>,
    ) -> Vec<(std::path::PathBuf, Vec<u8>)> {
        let out = crate::support::out_dir(&format!("web-{TAG}-{name}"));
        let mut plan = plan(TALL_PAGE, steps);
        plan.environment.reduced_motion = true;
        let bundle = run_plan(backend, &plan, &out).await.unwrap();
        bundle
            .checkpoints
            .into_iter()
            .map(|c| {
                let bytes = fs::read(&c.png).unwrap();
                (c.png, bytes)
            })
            .collect()
    }

    #[sync_test]
    #[async_test]
    async fn a_checkpoint_right_after_a_scroll_key_shows_the_scrolled_page() {
        let found = WebBackend::discover("chromium").await;
        let Some(backend) = found else {
            return;
        };
        let shots = shoot_tall_page(
            &backend,
            "settle",
            vec![
                checkpoint("top", false),
                press("End"),
                checkpoint("bottom", false),
                // Nothing is left to scroll, so a long pause cannot change what is shown.
                Step::Wait {
                    condition: WaitCondition::Delay { ms: LONG_PAUSE_MS },
                },
                checkpoint("bottom-later", false),
            ],
        )
        .await;
        assert_ne!(shots[0].1, shots[1].1, "End had not scrolled yet");
        assert_eq!(shots[1].1, shots[2].1, "the page was still moving");
    }

    #[sync_test]
    #[async_test]
    async fn a_full_page_checkpoint_is_as_tall_as_the_page() {
        let found = WebBackend::discover("chromium").await;
        let Some(backend) = found else {
            return;
        };
        let shots = shoot_tall_page(
            &backend,
            "full-page",
            vec![checkpoint("view", false), checkpoint("page", true)],
        )
        .await;
        assert_eq!(png_height(&shots[0].0), VIEWPORT[1]);
        assert_eq!(png_height(&shots[1].0), TALL_PAGE_HEIGHT);
    }

    /// A short page whose box turns red under the pointer, with a list that scrolls.
    const HOVER_PAGE: &str = "data:text/html,<style>%23box{height:100px;background:blue}\
        %23box:hover{background:red}%23list{height:100px;overflow:auto}</style>\
        <div id='box'></div><div id='list'><div style='height:900px;background:linear-gradient(green,yellow)'></div></div>";

    #[sync_test]
    #[async_test]
    async fn scrolling_an_element_does_not_hover_it() {
        let found = WebBackend::discover("chromium").await;
        let Some(backend) = found else {
            return;
        };
        let out = crate::support::out_dir(&format!("web-{TAG}-scroll-no-hover"));
        let mut plan = plan(
            HOVER_PAGE,
            vec![
                checkpoint("before", false),
                act(UiAction::Scroll {
                    target_id: Some("box".to_owned()),
                    dx: 0.0,
                    dy: 0.0,
                }),
                checkpoint("after-box", false),
                act(UiAction::Scroll {
                    target_id: Some("list".to_owned()),
                    dx: 0.0,
                    dy: 300.0,
                }),
                checkpoint("after-list", false),
                act(UiAction::ScrollToEnd {
                    target_id: Some("list".to_owned()),
                }),
                checkpoint("list-end", false),
                act(UiAction::ScrollTo {
                    target_id: Some("list".to_owned()),
                    x: 0.0,
                    y: 300.0,
                }),
                checkpoint("list-back", false),
            ],
        );
        plan.environment.reduced_motion = true;
        let bundle = run_plan(&backend, &plan, &out).await.unwrap();
        let bytes = |index: usize| fs::read(&bundle.checkpoints[index].png).unwrap();
        assert_eq!(bytes(0), bytes(1), "scrolling the box hovered it");
        assert_ne!(bytes(0), bytes(2), "the list did not scroll");
        assert_ne!(bytes(2), bytes(3), "scroll-to-end did not move the list");
        assert_eq!(
            bytes(2),
            bytes(4),
            "scroll-to did not go back to the offset"
        );
    }

    #[sync_test]
    #[async_test]
    async fn scroll_actions_reach_an_element_a_position_and_the_end() {
        let found = WebBackend::discover("chromium").await;
        let Some(backend) = found else {
            return;
        };
        let shots = shoot_tall_page(
            &backend,
            "scroll-actions",
            vec![
                checkpoint("top", false),
                act(UiAction::ScrollIntoView {
                    target_id: DEEP_ID.to_owned(),
                }),
                checkpoint("deep", false),
                act(UiAction::ScrollTo {
                    target_id: None,
                    x: 0.0,
                    y: 0.0,
                }),
                checkpoint("top-again", false),
                act(UiAction::ScrollToEnd { target_id: None }),
                checkpoint("end", false),
                act(UiAction::ScrollTo {
                    target_id: None,
                    x: 0.0,
                    y: 0.0,
                }),
                press("End"),
                checkpoint("end-by-key", false),
            ],
        )
        .await;
        assert_ne!(shots[0].1, shots[1].1, "the element was not scrolled to");
        assert_eq!(
            shots[0].1, shots[2].1,
            "scroll-to did not return to the top"
        );
        assert_ne!(shots[0].1, shots[3].1, "scroll-to-end did not move");
        assert_eq!(
            shots[3].1, shots[4].1,
            "scroll-to-end did not reach the key's end"
        );
    }
}
