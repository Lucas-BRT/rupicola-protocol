//! The Chromium backend as a live session, in both flavours. It needs a Chromium-family
//! browser in `PATH` and does nothing without one.

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
        flavour::{CaptureBackend as _, CdpBackend, CdpDiscovery as _, run_plan},
        session::{
            Capability, Environment, ErrorClass, Plan, Step, Target, UiTreeMode, WaitCondition,
        },
    };
    use rupicola_protocol::{SnapshotSource, ToolkitKind, WidgetKind};
    use std::{fs, sync::atomic::Ordering};

    const VIEWPORT: [u32; 2] = [800, 600];
    const SCALE: f64 = 2.0;
    const TIMEOUT_MS: u64 = 600;
    /// What the example's counter says once its script has run, before anything is added.
    const COUNTER_TEXT: &str = "2 remaining · 3 total";
    const CLOSED_PORT_URL: &str = "http://127.0.0.1:1/";

    fn plan(url: &str, mode: UiTreeMode, steps: Vec<Step>) -> Plan {
        let mut environment = Environment::new(VIEWPORT);
        environment.warmup_ms = 0;
        environment.ui_tree = mode;
        environment.timeout_ms = Some(TIMEOUT_MS * 10);
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

    fn checkpoint(id: &str) -> Step {
        Step::Checkpoint { id: id.to_owned() }
    }

    /// The width and height a PNG states.
    fn png_size(path: &std::path::Path) -> [u32; 2] {
        let header = fs::read(path).unwrap();
        [
            u32::from_be_bytes(header[16..20].try_into().unwrap()),
            u32::from_be_bytes(header[20..24].try_into().unwrap()),
        ]
    }

    #[sync_test]
    #[async_test]
    async fn one_live_page_waits_for_its_content_and_gives_a_tree_with_each_screenshot() {
        let found = CdpBackend::discover().await;
        let Some(backend) = found else {
            return;
        };
        assert!(backend.capabilities().contains(Capability::UiTree));
        let (url, stop) = crate::support::serve_example();
        let out = crate::support::out_dir(&format!("cdp-{TAG}-live"));
        let mut plan = plan(
            &url,
            UiTreeMode::Full,
            vec![
                Step::Wait {
                    condition: WaitCondition::Visible {
                        target_id: "add-btn".to_owned(),
                    },
                },
                Step::Wait {
                    condition: WaitCondition::Text {
                        text: COUNTER_TEXT.to_owned(),
                    },
                },
                checkpoint("page"),
            ],
        );
        plan.environment.scale = SCALE;
        let bundle = run_plan(&backend, &plan, &out).await.unwrap();
        stop.store(true, Ordering::Relaxed);
        let shot = &bundle.checkpoints[0];
        assert_eq!(
            png_size(&shot.png),
            [1600, 1200],
            "the scale applies to the screenshot"
        );
        let tree = shot.snapshot.as_ref().expect("the page's tree");
        assert_eq!(tree.toolkit, ToolkitKind::WebDom);
        assert_eq!(tree.source, SnapshotSource::Dom);
        assert_eq!(tree.viewport, VIEWPORT);
        assert!((tree.scale_factor - SCALE).abs() < f64::EPSILON);
        assert_eq!(tree.find_by_id("add-btn").unwrap().kind, WidgetKind::Button);
        assert_eq!(
            tree.find_by_id("new-task").unwrap().kind,
            WidgetKind::TextBox
        );
        assert_eq!(
            tree.find_by_id("counter").unwrap().label.as_deref(),
            Some(COUNTER_TEXT)
        );
        crate::support::keep_json(&out, "tree.json", &shot.snapshot);
        assert!(
            !out.join(".chromium-profile").exists(),
            "the browser's profile is removed with the session"
        );
        crate::support::finish(&out);
    }

    #[sync_test]
    #[async_test]
    async fn the_plan_decides_how_much_of_the_page_tree_is_kept() {
        let found = CdpBackend::discover().await;
        let Some(backend) = found else {
            return;
        };
        let (url, stop) = crate::support::serve_example();
        let out = crate::support::out_dir(&format!("cdp-{TAG}-ui-tree-modes"));
        let tree = |mode| {
            let plan = plan(&url, mode, vec![checkpoint("home")]);
            let dir = out.join(format!("{mode:?}").to_lowercase());
            let backend = &backend;
            async move {
                let _ = fs::remove_dir_all(&dir);
                run_plan(backend, &plan, &dir)
                    .await
                    .unwrap()
                    .checkpoints
                    .remove(0)
                    .snapshot
            }
        };
        assert_eq!(tree(UiTreeMode::Off).await, None);
        assert!(tree(UiTreeMode::Full).await.unwrap().title.is_some());
        let structure = tree(UiTreeMode::Structure).await.unwrap();
        stop.store(true, Ordering::Relaxed);
        assert_eq!(structure.title, None);
        let counter = structure.find_by_id("counter").unwrap();
        assert_eq!(counter.label, None, "structure keeps no text");
        assert!(counter.bounds.is_valid());
        crate::support::finish(&out);
    }

    #[sync_test]
    #[async_test]
    async fn an_unreachable_page_and_a_missing_element_are_target_errors() {
        let found = CdpBackend::discover().await;
        let Some(backend) = found else {
            return;
        };
        let out = crate::support::out_dir(&format!("cdp-{TAG}-errors"));
        let closed = plan(CLOSED_PORT_URL, UiTreeMode::Off, vec![checkpoint("x")]);
        let error = run_plan(&backend, &closed, &out).await.unwrap_err();
        assert_eq!(error.class, ErrorClass::Target, "{}", error.message);
        let (url, stop) = crate::support::serve_example();
        let mut missing = plan(
            &url,
            UiTreeMode::Off,
            vec![Step::Wait {
                condition: WaitCondition::Visible {
                    target_id: "nope".to_owned(),
                },
            }],
        );
        missing.environment.timeout_ms = Some(TIMEOUT_MS);
        let error = run_plan(&backend, &missing, &out).await.unwrap_err();
        stop.store(true, Ordering::Relaxed);
        assert_eq!(error.class, ErrorClass::Target);
        assert!(error.message.contains("timed out"), "{}", error.message);
        assert!(
            !out.join(".chromium-profile").exists(),
            "a failed session still removes its profile"
        );
        crate::support::finish(&out);
    }
}
