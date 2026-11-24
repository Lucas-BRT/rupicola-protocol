//! Annotating a session's screenshot and clip, and the Android session, which needs no
//! device to report that it has none; both in both flavours.

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

    use rupicola_capture::flavour::{
        self, AndroidAdbBackend, annotate_png, annotate_video, run_plan,
    };
    use rupicola_capture::session::{Environment, ErrorClass, Plan, Step, Target, WaitCondition};
    use rupicola_protocol::{AnnotationBox, AnnotationShape, AnnotationStyle, Rect};
    use std::fs;

    const VIEWPORT: [u32; 2] = [320, 240];
    const WARMUP_MS: u64 = 100;
    /// Display number of the annotation test; distinct from every other test.
    #[cfg(unix)]
    const ANNOTATE_DISPLAY: u16 = 94 - SLOT;
    /// Pause that gives the clip some length.
    #[cfg(unix)]
    const CLIP_MS: u64 = 600;
    /// How long the placeholder target lives.
    #[cfg(unix)]
    const TARGET_SECONDS: &str = "30";

    #[cfg(unix)]
    #[sync_test]
    #[async_test]
    async fn annotates_a_session_screenshot_and_clip() {
        use rupicola_capture::flavour::LinuxX11Backend;
        let available = |tool: &str| {
            std::process::Command::new("which")
                .arg(tool)
                .output()
                .is_ok_and(|o| o.status.success())
        };
        if !available("Xvfb") || !available("ffmpeg") {
            return;
        }
        let out_dir = std::env::temp_dir().join(format!("rupicola-capture-{TAG}-annotate-session"));
        let _ = fs::remove_dir_all(&out_dir);
        let mut environment = Environment::new(VIEWPORT);
        environment.warmup_ms = WARMUP_MS;
        let plan = Plan {
            target: Target {
                command: "sleep".to_owned(),
                args: vec![TARGET_SECONDS.to_owned()],
            },
            environment,
            steps: vec![
                Step::StartRecording,
                Step::Wait {
                    condition: WaitCondition::Delay { ms: CLIP_MS },
                },
                Step::Checkpoint {
                    id: "shot".to_owned(),
                },
                Step::StopRecording {
                    id: "clip".to_owned(),
                },
            ],
            allow_degraded: false,
        };
        let bundle = run_plan(
            &LinuxX11Backend {
                display_num: ANNOTATE_DISPLAY,
            },
            &plan,
            &out_dir,
        )
        .await
        .unwrap();
        let boxes = [AnnotationBox {
            bounds: Rect {
                x: 24.0,
                y: 66.0,
                width: 100.0,
                height: 38.0,
            },
            label: "note-entry".into(),
            shape: AnnotationShape::Rect,
            style: AnnotationStyle::default(),
            time_range: None,
            keyframes: vec![],
        }];

        let annotated_png = out_dir.join("shot-annotated.png");
        annotate_png(&bundle.checkpoints[0].png, &annotated_png, &boxes, 1.0)
            .await
            .unwrap();
        let annotated_mp4 = out_dir.join("clip-annotated.mp4");
        annotate_video(&bundle.clips[0].mp4, &annotated_mp4, &boxes, 1.0)
            .await
            .unwrap();

        assert!(annotated_png.is_file());
        assert!(annotated_mp4.is_file());
        let _ = fs::remove_dir_all(&out_dir);
    }

    #[sync_test]
    #[async_test]
    async fn android_session_reports_a_missing_adb_or_device_as_an_environment_error() {
        let backend = AndroidAdbBackend::default();
        assert_eq!(flavour::CaptureBackend::id(&backend), "mobile-android-adb");
        let target = Target {
            command: String::new(),
            args: vec![],
        };
        let out = std::env::temp_dir().join(format!("rupicola-capture-{TAG}-android-out"));
        match flavour::CaptureBackend::start(&backend, &target, &Environment::new(VIEWPORT), &out)
            .await
        {
            // A device is attached: the session must be usable and end cleanly.
            Ok(session) => session.finish().await.unwrap(),
            Err(error) => assert_eq!(error.class, ErrorClass::Environment),
        }
        let _ = fs::remove_dir_all(&out);
    }
}
