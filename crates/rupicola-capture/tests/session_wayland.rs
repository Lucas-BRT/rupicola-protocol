//! The nested-Wayland backend as a session, in both flavours.

#![cfg(unix)]

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

    /// Moves the displays and names this flavour's tests use away from the other flavour's,
    /// so both can run at once.
    #[maybe_async_cfg::only_if(sync)]
    const SLOT: u16 = 0;
    #[maybe_async_cfg::only_if(async)]
    const SLOT: u16 = 10;
    #[maybe_async_cfg::only_if(sync)]
    const TAG: &str = "sync";
    #[maybe_async_cfg::only_if(async)]
    const TAG: &str = "async";

    use rupicola_capture::{
        flavour::{LinuxWaylandBackend, run_plan},
        session::{Environment, Plan, Step, Target, WaitCondition},
    };
    use rupicola_protocol::UiAction;
    use std::{fs, process::Command};

    const VIEWPORT: [u32; 2] = [800, 600];
    /// Where the example editor's "File" menu sits at this viewport.
    const FILE_MENU: (f64, f64) = (297.0, 124.0);
    /// Time the app gets to draw its first frame.
    const WARMUP_MS: u64 = 3000;
    /// Time the menu gets to open after the click.
    const MENU_MS: u64 = 800;
    /// A blank frame compresses to almost nothing; a drawn window does not.
    const MIN_FRAME_BYTES: u64 = 20_000;
    /// Unusual display number, so the test cannot clash with a developer's own Xvfb.
    const TEST_DISPLAY: u16 = 87 - SLOT;

    fn available(tool: &str) -> bool {
        Command::new("which")
            .arg(tool)
            .output()
            .is_ok_and(|output| output.status.success())
    }

    #[sync_test]
    #[async_test]
    async fn a_cosmic_app_is_captured_before_and_after_a_click() {
        if [
            "Xvfb",
            "ffmpeg",
            "python3",
            "dbus-daemon",
            "cosmic-comp",
            "cosmic-edit",
        ]
        .into_iter()
        .any(|tool| !available(tool))
        {
            return;
        }
        let out = std::env::temp_dir().join(format!("rupicola-capture-{TAG}-session-wayland"));
        let _ = fs::remove_dir_all(&out);
        let mut environment = Environment::new(VIEWPORT);
        environment.warmup_ms = WARMUP_MS;
        let plan = Plan {
            target: Target {
                command: "cosmic-edit".to_owned(),
                args: Vec::new(),
            },
            environment,
            steps: vec![
                Step::Checkpoint {
                    id: "closed".to_owned(),
                },
                Step::StartRecording,
                Step::Action {
                    action: UiAction::ClickAt {
                        x: FILE_MENU.0,
                        y: FILE_MENU.1,
                    },
                },
                Step::Wait {
                    condition: WaitCondition::Delay { ms: MENU_MS },
                },
                Step::Checkpoint {
                    id: "open".to_owned(),
                },
                Step::StopRecording {
                    id: "walk".to_owned(),
                },
            ],
            allow_degraded: false,
        };
        let backend = LinuxWaylandBackend {
            display_num: TEST_DISPLAY,
            ..LinuxWaylandBackend::default()
        };
        let bundle = run_plan(&backend, &plan, &out).await.unwrap();
        assert_eq!(bundle.checkpoints.len(), 2);
        for checkpoint in &bundle.checkpoints {
            assert!(fs::metadata(&checkpoint.png).unwrap().len() > MIN_FRAME_BYTES);
        }
        assert_ne!(
            fs::read(&bundle.checkpoints[0].png).unwrap(),
            fs::read(&bundle.checkpoints[1].png).unwrap(),
            "the click changed nothing on screen"
        );
        assert!(bundle.clips[0].mp4.is_file());
        if let Some(keep) = std::env::var_os("RUPICOLA_KEEP_CAPTURES") {
            let _ = fs::create_dir_all(&keep);
            for checkpoint in &bundle.checkpoints {
                let _ = fs::copy(
                    &checkpoint.png,
                    std::path::Path::new(&keep).join(format!("{}.png", checkpoint.id)),
                );
            }
        }
        let _ = fs::remove_dir_all(&out);
    }

    #[sync_test]
    #[async_test]
    async fn a_missing_compositor_is_an_environment_error() {
        if !available("Xvfb") {
            return;
        }
        let out =
            std::env::temp_dir().join(format!("rupicola-capture-{TAG}-session-wayland-missing"));
        let plan = Plan {
            target: Target {
                command: "cosmic-edit".to_owned(),
                args: Vec::new(),
            },
            environment: Environment::new(VIEWPORT),
            steps: Vec::new(),
            allow_degraded: false,
        };
        let backend = LinuxWaylandBackend {
            compositor: "no-such-compositor".to_owned(),
            display_num: TEST_DISPLAY - 1,
            ..LinuxWaylandBackend::default()
        };
        let error = run_plan(&backend, &plan, &out).await.unwrap_err();
        assert_eq!(
            error.class,
            rupicola_capture::session::ErrorClass::Environment
        );
        let _ = fs::remove_dir_all(&out);
    }
}
