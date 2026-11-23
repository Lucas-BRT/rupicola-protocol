//! The X11 backend as a session, in both flavours: one display serves several checkpoints and a clip.

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
        flavour::{LinuxX11Backend, run_plan},
        session::{Environment, Plan, Step, Target, WaitCondition},
    };
    use std::{fs, process::Command};

    /// Display numbers, one per test since the tests run in parallel, and distinct from the
    /// one the legacy test uses.
    const CLIP_DISPLAY: u16 = 93 - SLOT;
    const MISSING_TARGET_DISPLAY: u16 = 92 - SLOT;
    const UNSAFE_ID_DISPLAY: u16 = 91 - SLOT;
    const VIEWPORT: [u32; 2] = [320, 240];
    /// Pause between the two checkpoints, so the clip has some length.
    const CLIP_MS: u64 = 600;
    /// How long the placeholder target lives.
    const TARGET_SECONDS: &str = "30";

    fn available(tool: &str) -> bool {
        Command::new("which")
            .arg(tool)
            .output()
            .is_ok_and(|output| output.status.success())
    }

    #[sync_test]
    #[async_test]
    async fn one_display_serves_two_checkpoints_and_a_clip() {
        if !available("Xvfb") || !available("ffmpeg") {
            return;
        }
        let out = std::env::temp_dir().join(format!("rupicola-capture-{TAG}-session-x11"));
        let _ = fs::remove_dir_all(&out);
        let mut environment = Environment::new(VIEWPORT);
        environment.warmup_ms = 100;
        let plan = Plan {
            target: Target {
                command: "sleep".to_owned(),
                args: vec![TARGET_SECONDS.to_owned()],
            },
            environment,
            steps: vec![
                Step::Checkpoint {
                    id: "before".to_owned(),
                },
                Step::StartRecording,
                Step::Wait {
                    condition: WaitCondition::Delay { ms: CLIP_MS },
                },
                Step::Checkpoint {
                    id: "after".to_owned(),
                },
                Step::StopRecording {
                    id: "walk".to_owned(),
                },
            ],
            allow_degraded: false,
        };
        let bundle = run_plan(
            &LinuxX11Backend {
                display_num: CLIP_DISPLAY,
            },
            &plan,
            &out,
        )
        .await
        .unwrap();
        assert_eq!(bundle.checkpoints.len(), 2);
        assert!(bundle.checkpoints.iter().all(|c| c.png.is_file()));
        assert!(bundle.clips[0].mp4.is_file());
        assert!(!out.join(".recording.mp4").exists());
        let _ = fs::remove_dir_all(&out);
    }

    #[sync_test]
    #[async_test]
    async fn a_missing_target_is_a_target_error_and_leaves_nothing_running() {
        if !available("Xvfb") {
            return;
        }
        let out = std::env::temp_dir().join(format!("rupicola-capture-{TAG}-session-x11-missing"));
        let plan = Plan {
            target: Target {
                command: "/nonexistent/rupicola-target".to_owned(),
                args: Vec::new(),
            },
            environment: Environment::new(VIEWPORT),
            steps: Vec::new(),
            allow_degraded: false,
        };
        let error = run_plan(
            &LinuxX11Backend {
                display_num: MISSING_TARGET_DISPLAY,
            },
            &plan,
            &out,
        )
        .await
        .unwrap_err();
        assert_eq!(error.class, rupicola_capture::session::ErrorClass::Target);
        let _ = fs::remove_dir_all(&out);
    }

    #[sync_test]
    #[async_test]
    async fn an_unsafe_id_is_refused_before_anything_starts() {
        let plan = Plan {
            target: Target {
                command: "sleep".to_owned(),
                args: Vec::new(),
            },
            environment: Environment::new(VIEWPORT),
            steps: vec![Step::Checkpoint {
                id: "../escape".to_owned(),
            }],
            allow_degraded: false,
        };
        let out = std::env::temp_dir().join(format!("rupicola-capture-{TAG}-session-x11-id"));
        let error = run_plan(
            &LinuxX11Backend {
                display_num: UNSAFE_ID_DISPLAY,
            },
            &plan,
            &out,
        )
        .await
        .unwrap_err();
        assert!(error.message.contains("valid id"));
        assert!(!out.exists());
    }
}
