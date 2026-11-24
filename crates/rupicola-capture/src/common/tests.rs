//! The unit tests of the session contract, written once and generated for each flavour.

#[cfg(test)]
#[maybe_async_cfg::maybe(
    idents(flavour(sync = "sync", async = "asynchronous")),
    sync(feature = "sync", drop_attrs(async_test)),
    async(feature = "async", drop_attrs(sync_test))
)]
mod flavour {
    use crate::common::runner::flavour::run_plan;
    use crate::common::traits::flavour::{CaptureBackend, CaptureSession};
    use crate::session::{
        Capabilities, CaptureError, Checkpoint, Clip, Environment, Target, UiTreeMode,
        WaitCondition,
    };
    use crate::session::{Capability, ErrorClass, Plan, Step};
    #[allow(unused_imports)]
    use core::prelude::v1::test as sync_test;
    use rupicola_protocol::{
        Rect, SnapshotSource, ToolkitKind, UiAction, UiSnapshot, WidgetKind, WidgetNode,
        WidgetState,
    };
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    #[allow(unused_imports)]
    use tokio::test as async_test;

    const SECRET: &str = "ana@example.com";

    /// A tree every fake backend returns, whether or not the plan asked for it.
    fn snapshot() -> UiSnapshot {
        UiSnapshot {
            protocol_version: 1,
            app_id: "app".to_owned(),
            toolkit: ToolkitKind::WebDom,
            title: Some(SECRET.to_owned()),
            viewport: [800, 600],
            scale_factor: 1.0,
            timestamp_ms: 0,
            source: SnapshotSource::Dom,
            root: WidgetNode {
                id: "root".to_owned(),
                role: "div".to_owned(),
                kind: WidgetKind::Group,
                label: Some(SECRET.to_owned()),
                value: Some(SECRET.to_owned()),
                bounds: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 800.0,
                    height: 600.0,
                },
                state: WidgetState {
                    visible: true,
                    ..WidgetState::default()
                },
                style_hint: None,
                children: Vec::new(),
            },
        }
    }

    type Calls = Arc<Mutex<Vec<String>>>;

    /// Records every call so tests can check order and cleanup.
    #[derive(Default)]
    struct Journal(Calls);

    struct Fake {
        capabilities: Capabilities,
        journal: Calls,
        fail_on: Option<&'static str>,
    }

    struct FakeSession {
        journal: Calls,
        fail_on: Option<&'static str>,
    }

    impl FakeSession {
        fn note(&self, entry: String) -> Result<(), CaptureError> {
            let fails = self.fail_on.is_some_and(|prefix| entry.starts_with(prefix));
            self.journal.lock().unwrap().push(entry);
            if fails {
                Err(CaptureError::target("boom"))
            } else {
                Ok(())
            }
        }
    }

    #[cfg_attr(feature = "async", async_trait::async_trait)]
    impl CaptureSession for FakeSession {
        async fn perform(&mut self, action: &UiAction) -> Result<(), CaptureError> {
            self.note(format!("perform {action:?}"))
        }
        async fn wait_for(&mut self, condition: &WaitCondition) -> Result<(), CaptureError> {
            self.note(format!("wait {condition:?}"))
        }
        async fn screenshot(&mut self, id: &str) -> Result<Checkpoint, CaptureError> {
            self.note(format!("screenshot {id}"))?;
            Ok(Checkpoint {
                id: id.to_owned(),
                png: PathBuf::from(format!("{id}.png")),
                snapshot: Some(snapshot()),
            })
        }
        async fn start_recording(&mut self) -> Result<(), CaptureError> {
            self.note("start_recording".to_owned())
        }
        async fn stop_recording(&mut self, id: &str) -> Result<Clip, CaptureError> {
            self.note(format!("stop_recording {id}"))?;
            Ok(Clip {
                id: id.to_owned(),
                mp4: PathBuf::from(format!("{id}.mp4")),
            })
        }
        async fn finish(self: Box<Self>) -> Result<(), CaptureError> {
            self.note("finish".to_owned())
        }
    }

    #[cfg_attr(feature = "async", async_trait::async_trait)]
    impl CaptureBackend for Fake {
        fn id(&self) -> &'static str {
            "fake"
        }
        fn capabilities(&self) -> Capabilities {
            self.capabilities
        }
        async fn start(
            &self,
            _: &Target,
            _: &Environment,
            _: &Path,
        ) -> Result<Box<dyn CaptureSession>, CaptureError> {
            self.journal.lock().unwrap().push("start".to_owned());
            Ok(Box::new(FakeSession {
                journal: Arc::clone(&self.journal),
                fail_on: self.fail_on,
            }))
        }
    }

    const FULL: Capabilities = Capabilities::of(&Capability::ALL);
    const SCREENSHOT_ONLY: Capabilities = Capabilities::of(&[]);
    const VIEWPORT: [u32; 2] = [800, 600];

    fn plan(steps: Vec<Step>, allow_degraded: bool) -> Plan {
        Plan {
            target: Target {
                command: "app".to_owned(),
                args: Vec::new(),
            },
            environment: Environment::new(VIEWPORT),
            steps,
            allow_degraded,
        }
    }

    fn flow() -> Vec<Step> {
        vec![
            Step::Checkpoint {
                id: "before".to_owned(),
                full_page: false,
            },
            Step::StartRecording,
            Step::Action {
                action: UiAction::Click {
                    target_id: "go".to_owned(),
                },
            },
            Step::Checkpoint {
                id: "after".to_owned(),
                full_page: false,
            },
            Step::StopRecording {
                id: "walk".to_owned(),
            },
        ]
    }

    fn fake(capabilities: Capabilities, fail_on: Option<&'static str>) -> (Fake, Journal) {
        let journal = Journal::default();
        let backend = Fake {
            capabilities,
            journal: Arc::clone(&journal.0),
            fail_on,
        };
        (backend, journal)
    }

    #[sync_test]
    #[async_test]
    async fn one_session_serves_every_checkpoint() {
        let (backend, journal) = fake(FULL, None);
        let bundle = run_plan(&backend, &plan(flow(), false), Path::new("out"))
            .await
            .unwrap();
        assert_eq!(bundle.checkpoints.len(), 2);
        assert_eq!(bundle.clips.len(), 1);
        let calls = journal.0.lock().unwrap();
        assert_eq!(calls.iter().filter(|call| *call == "start").count(), 1);
        assert_eq!(calls.last().map(String::as_str), Some("finish"));
    }

    #[sync_test]
    #[async_test]
    async fn a_plan_the_backend_cannot_honour_is_refused_before_starting() {
        let (backend, journal) = fake(SCREENSHOT_ONLY, None);
        let error = run_plan(&backend, &plan(flow(), false), Path::new("out"))
            .await
            .unwrap_err();
        assert_eq!(error.class, ErrorClass::Environment);
        assert!(error.message.contains("actions") && error.message.contains("video"));
        assert!(journal.0.lock().unwrap().is_empty());
    }

    #[sync_test]
    #[async_test]
    async fn a_degraded_plan_skips_what_is_missing_and_says_so() {
        let (backend, _) = fake(SCREENSHOT_ONLY, None);
        let bundle = run_plan(&backend, &plan(flow(), true), Path::new("out"))
            .await
            .unwrap();
        assert_eq!(bundle.checkpoints.len(), 2);
        assert_eq!(bundle.clips.len(), 0);
        assert_eq!(bundle.warnings.len(), 2);
    }

    #[sync_test]
    #[async_test]
    async fn the_session_is_finished_after_a_failed_step() {
        let (backend, journal) = fake(FULL, Some("perform"));
        let error = run_plan(&backend, &plan(flow(), false), Path::new("out"))
            .await
            .unwrap_err();
        assert_eq!(error.class, ErrorClass::Target);
        assert_eq!(
            journal.0.lock().unwrap().last().map(String::as_str),
            Some("finish")
        );
    }

    #[test]
    fn coordinate_actions_need_only_the_pointer() {
        let click = Step::Action {
            action: UiAction::ClickAt { x: 1.0, y: 2.0 },
        };
        let named = Step::Action {
            action: UiAction::Click {
                target_id: "go".to_owned(),
            },
        };
        let pointer = Capabilities::of(&[Capability::Pointer]);
        assert_eq!(pointer.missing(click.needs()), []);
        assert_eq!(pointer.missing(named.needs()), [Capability::Actions]);
        assert_eq!(FULL.missing(click.needs()), []);
        let full_actions = Capabilities::of(&[Capability::Actions]);
        assert_eq!(full_actions.missing(click.needs()), []);
    }

    #[test]
    fn plans_round_trip_through_json() {
        let original = plan(flow(), true);
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(serde_json::from_str::<Plan>(&json).unwrap(), original);
    }

    fn with_tree(mode: UiTreeMode, steps: Vec<Step>, allow_degraded: bool) -> Plan {
        let mut plan = plan(steps, allow_degraded);
        plan.environment.ui_tree = mode;
        plan
    }

    fn one_checkpoint() -> Vec<Step> {
        vec![Step::Checkpoint {
            id: "home".to_owned(),
            full_page: false,
        }]
    }

    #[sync_test]
    #[async_test]
    async fn the_plan_decides_how_much_of_the_tree_a_checkpoint_keeps() {
        let (backend, _) = fake(FULL, None);
        let run = |mode| {
            let plan = with_tree(mode, one_checkpoint(), false);
            let backend = &backend;
            async move {
                run_plan(backend, &plan, Path::new("out"))
                    .await
                    .unwrap()
                    .checkpoints
                    .remove(0)
                    .snapshot
            }
        };
        assert_eq!(
            run(UiTreeMode::Off).await,
            None,
            "a backend's tree is dropped when off"
        );
        let full = run(UiTreeMode::Full).await.unwrap();
        assert_eq!(full.title.as_deref(), Some(SECRET));
        assert_eq!(full.root.label.as_deref(), Some(SECRET));
        let structure = run(UiTreeMode::Structure).await.unwrap();
        assert_eq!(structure.title, None);
        assert_eq!(structure.root.label, None);
        assert_eq!(structure.root.value, None);
        assert_eq!(structure.root.kind, WidgetKind::Group);
    }

    #[sync_test]
    #[async_test]
    async fn asking_for_a_tree_a_backend_cannot_give_refuses_the_plan_or_degrades_it() {
        let (backend, _) = fake(SCREENSHOT_ONLY, None);
        let strict = with_tree(UiTreeMode::Full, one_checkpoint(), false);
        let error = run_plan(&backend, &strict, Path::new("out"))
            .await
            .unwrap_err();
        assert_eq!(error.class, ErrorClass::Environment);
        assert!(error.message.contains("ui-tree"), "{}", error.message);
        let lenient = with_tree(UiTreeMode::Full, one_checkpoint(), true);
        let bundle = run_plan(&backend, &lenient, Path::new("out"))
            .await
            .unwrap();
        assert_eq!(bundle.checkpoints.len(), 1, "the screenshot is still taken");
        assert_eq!(bundle.warnings, ["backend fake lacks ui-tree; skipped"]);
    }

    #[sync_test]
    #[async_test]
    async fn a_full_page_checkpoint_a_backend_cannot_take_refuses_the_plan_or_is_skipped() {
        let (backend, _) = fake(SCREENSHOT_ONLY, None);
        let steps = || {
            vec![Step::Checkpoint {
                id: "page".to_owned(),
                full_page: true,
            }]
        };
        let error = run_plan(&backend, &plan(steps(), false), Path::new("out"))
            .await
            .unwrap_err();
        assert_eq!(error.class, ErrorClass::Environment);
        assert!(error.message.contains("full-page"), "{}", error.message);
        let bundle = run_plan(&backend, &plan(steps(), true), Path::new("out"))
            .await
            .unwrap();
        assert_eq!(bundle.checkpoints.len(), 0);
        assert_eq!(bundle.warnings, ["backend fake lacks full-page; skipped"]);
    }

    #[test]
    fn a_plan_without_a_tree_mode_means_off() {
        let json =
            r#"{"target":{"command":"app"},"environment":{"viewport":[800,600]},"steps":[]}"#;
        let plan: Plan = serde_json::from_str(json).unwrap();
        assert_eq!(plan.environment.ui_tree, UiTreeMode::Off);
        assert!(!plan.needs().contains(Capability::UiTree));
        let asked = with_tree(UiTreeMode::Structure, Vec::new(), false);
        assert!(asked.needs().contains(Capability::UiTree));
        assert_eq!(
            serde_json::to_value(UiTreeMode::Structure).unwrap(),
            "structure"
        );
    }

    #[sync_test]
    #[async_test]
    async fn environment_probe_reports_all_known_backends() {
        let report = super::super::probe::flavour::probe_environment().await;
        assert_ne!(report.len(), 0);

        for backend in &report {
            assert_ne!(backend.id, "");
            assert_ne!(backend.label, "");
            assert_ne!(backend.install_guidance, "");
            if backend.available {
                assert_eq!(backend.missing_dependencies, Vec::<String>::new());
                assert_ne!(backend.detected_binaries.len(), 0);
            } else {
                assert_ne!(backend.missing_dependencies.len(), 0);
            }
        }
        assert!(report.iter().any(|b| b.id == "web-firefox"));
        assert!(report.iter().any(|b| b.id == "web-safari"));
    }

    #[sync_test]
    #[async_test]
    async fn web_discovery_validates_supported_engines() {
        use super::super::web::flavour::WebDiscovery as _;
        use crate::backend::WebBackend;

        assert!(WebBackend::is_supported_engine("chromium"));
        assert!(WebBackend::is_supported_engine("firefox"));
        assert!(WebBackend::is_supported_engine("webkit"));
        assert!(!WebBackend::is_supported_engine("unknown-browser"));
        assert!(WebBackend::discover("unknown-browser").await.is_none());
        assert!(
            WebBackend::discover_any(&["unknown-browser"])
                .await
                .is_none()
        );
    }

    #[test]
    fn network_policy_public_only_blocks_private_and_allows_public() {
        let policy = crate::session::NetworkPolicy::public_only();
        for blocked in [
            "http://127.0.0.1:8080/health",
            "http://localhost/admin",
            "http://sub.localhost/",
            "http://169.254.169.254/latest/meta-data/",
            "http://10.0.0.1/",
            "http://192.168.1.1/",
            "http://[::1]:8080/",
            "http://[::ffff:127.0.0.1]/",
            "file:///etc/passwd",
            "http://user:pass@93.184.216.34/",
        ] {
            let err = policy.check_url_syntax(blocked).unwrap_err();
            assert_eq!(err.class, ErrorClass::Target, "{blocked}");
        }
        assert!(
            policy
                .check_url_syntax("https://93.184.216.34/page")
                .is_ok()
        );
        assert!(
            crate::session::NetworkPolicy::Unrestricted
                .check_url_syntax("http://127.0.0.1:8080")
                .is_ok()
        );
    }

    #[cfg(unix)]
    #[test]
    fn cdp_backend_no_sandbox_is_opt_in() {
        let env = Environment::new([800, 600]);
        let default_cmd =
            crate::backend::CdpBackend::new("chromium").launch(&env, Path::new("/tmp/p"));
        let default_args: Vec<_> = default_cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(!default_args.iter().any(|a| a == "--no-sandbox"));

        let opt_in_cmd = crate::backend::CdpBackend::new("chromium")
            .with_no_sandbox(true)
            .launch(&env, Path::new("/tmp/p"));
        let opt_in_args: Vec<_> = opt_in_cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert!(opt_in_args.iter().any(|a| a == "--no-sandbox"));
    }
}
