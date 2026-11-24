//! The Android backend as a session, on a real device when one is attached.
//!
//! The checks are about structure and never about what the screen says. The screenshot goes to
//! a temporary directory that is removed, unless `RUPICOLA_KEEP_CAPTURES` names a directory to
//! keep it in, to be looked at.

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
        flavour::{AndroidAdbBackend, run_plan},
        session::{Environment, Plan, Step, Target, UiTreeMode},
    };
    use rupicola_protocol::{SnapshotSource, ToolkitKind, UiSnapshot, WidgetNode};
    use std::{
        fs,
        process::Command,
        sync::{Mutex, PoisonError},
    };

    /// One device serves one capture at a time: UI Automator takes a single connection, and the
    /// tests of both flavours run at once.
    static DEVICE: Mutex<()> = Mutex::new(());

    const VIEWPORT: [u32; 2] = [0, 0];
    /// The screenshot and the tree are taken moments apart; a pixel of rounding is allowed.
    const ROUNDING_PX: f64 = 1.5;

    /// Whether a device is attached; without one the tests have nothing to run on.
    fn device_attached() -> bool {
        Command::new("adb")
            .arg("get-state")
            .output()
            .is_ok_and(|output| output.status.success())
    }

    fn plan(mode: UiTreeMode) -> Plan {
        let mut environment = Environment::new(VIEWPORT);
        environment.ui_tree = mode;
        Plan {
            target: Target {
                command: String::new(),
                args: Vec::new(),
            },
            environment,
            steps: vec![Step::Checkpoint {
                id: "screen".to_owned(),
                full_page: false,
            }],
            allow_degraded: false,
        }
    }

    /// The tree and the size of the screenshot taken with it.
    async fn capture(mode: UiTreeMode) -> (Option<UiSnapshot>, [u32; 2]) {
        let out = crate::support::out_dir(&format!("android-{TAG}-{mode:?}").to_lowercase());
        let backend = AndroidAdbBackend::default();
        let mut bundle = run_plan(&backend, &plan(mode), &out).await.unwrap();
        let checkpoint = bundle.checkpoints.remove(0);
        let header = fs::read(&checkpoint.png).unwrap();
        let size = [
            u32::from_be_bytes(header[16..20].try_into().unwrap()),
            u32::from_be_bytes(header[20..24].try_into().unwrap()),
        ];
        crate::support::keep_json(&out, "tree.json", &checkpoint.snapshot);
        crate::support::finish(&out);
        (checkpoint.snapshot, size)
    }

    fn every_node(node: &WidgetNode, visit: &mut dyn FnMut(&WidgetNode)) {
        visit(node);
        for child in &node.children {
            every_node(child, visit);
        }
    }

    #[sync_test]
    #[async_test]
    #[allow(
        clippy::await_holding_lock,
        reason = "holding the lock across the capture is how the device is shared one test at a time"
    )]
    async fn the_screen_comes_with_a_tree_that_lines_up_with_its_screenshot() {
        if !device_attached() {
            return;
        }
        let _device = DEVICE.lock().unwrap_or_else(PoisonError::into_inner);
        let (tree, size) = capture(UiTreeMode::Structure).await;
        let tree = tree.expect("a view hierarchy");
        assert_eq!(tree.toolkit, ToolkitKind::Android);
        assert_eq!(tree.source, SnapshotSource::Uiautomator);
        assert!(tree.scale_factor > 0.0);
        for (logical, pixels) in tree.viewport.iter().zip(size) {
            let back = f64::from(*logical) * tree.scale_factor;
            assert!(
                (back - f64::from(pixels)).abs() <= ROUNDING_PX,
                "viewport {:?} at {} does not match a {size:?} screenshot",
                tree.viewport,
                tree.scale_factor
            );
        }
        let mut nodes = 0;
        let mut text_left = false;
        let mut outside = false;
        every_node(&tree.root, &mut |node| {
            nodes += 1;
            text_left |= node.label.is_some() || node.value.is_some();
            let [x, y, width, height] = node.bounds.to_pixel_box(tree.scale_factor);
            outside |= x + width > size[0] + 2 || y + height > size[1] + 2;
        });
        assert!(nodes >= 1);
        assert!(!text_left, "the structure mode kept some text");
        assert!(!outside, "a node lies outside the screenshot");
        assert_eq!(tree.title, None);
    }

    #[sync_test]
    #[async_test]
    #[allow(
        clippy::await_holding_lock,
        reason = "holding the lock across the capture is how the device is shared one test at a time"
    )]
    async fn the_plan_decides_whether_the_device_is_asked_at_all() {
        if !device_attached() {
            return;
        }
        let _device = DEVICE.lock().unwrap_or_else(PoisonError::into_inner);
        assert_eq!(capture(UiTreeMode::Off).await.0, None);
        let (full, _) = capture(UiTreeMode::Full).await;
        assert!(full.is_some());
    }
}
