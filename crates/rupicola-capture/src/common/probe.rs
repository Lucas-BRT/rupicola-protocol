//! Host OS dependency and capability probing across all capture backends, generated for
//! both `sync` and `asynchronous` flavours.

#[maybe_async_cfg::maybe(
    idents(
        flavour(sync = "sync", async = "asynchronous"),
        rt(sync = "rt_sync", async = "rt_async")
    ),
    sync(feature = "sync"),
    async(feature = "async")
)]
pub(crate) mod flavour {
    use crate::backend::WebBackend;
    use crate::backend::android_adb::ADB;
    use crate::backend::cdp::BROWSER_CANDIDATES;
    #[cfg(unix)]
    use crate::backend::gtk_broadway::BROADWAY_DAEMON;
    use crate::backend::tool::Tool;
    use crate::common::rt::Runtime;
    use crate::session::{BackendKind, BackendReport, Capabilities, Capability};
    use std::path::PathBuf;

    const NODE_TOOL: &str = "node";
    const FFMPEG_TOOL: &str = "ffmpeg";
    #[cfg(unix)]
    const XVFB_TOOL: &str = "Xvfb";
    #[cfg(unix)]
    const PYTHON3_TOOL: &str = "python3";
    #[cfg(unix)]
    const DBUS_DAEMON_TOOL: &str = "dbus-daemon";
    #[cfg(unix)]
    const WAYLAND_COMPOSITORS: [&str; 2] = ["cosmic-comp", "weston"];

    const PLAYWRIGHT_DEP: &str = "playwright (RUPICOLA_PLAYWRIGHT_DIR)";
    const PLAYWRIGHT_GUIDANCE: &str = "Install Node.js and Playwright (`npm install playwright`), then set RUPICOLA_PLAYWRIGHT_DIR (https://playwright.dev/docs/intro).";
    const CHROMIUM_DEP: &str = "chromium | google-chrome | brave | microsoft-edge";
    const CHROMIUM_GUIDANCE: &str = "Install a Chromium-family browser using your OS package manager (e.g. `apt install chromium`, `brew install --cask chromium`) or from https://www.chromium.org/getting-involved/download-chromium/.";
    #[cfg(unix)]
    const X11_GUIDANCE: &str = "Install Xvfb, ffmpeg, python3, libX11, and libXtst (e.g. `apt install xvfb ffmpeg python3 libx11-6 libxtst6`).";
    #[cfg(unix)]
    const WAYLAND_GUIDANCE: &str =
        "Install Xvfb, ffmpeg, python3, dbus-daemon, and cosmic-comp or weston.";
    #[cfg(unix)]
    const BROADWAY_GUIDANCE: &str = "Install gtk4-broadwayd (GTK4) and configure Playwright (Node.js + RUPICOLA_PLAYWRIGHT_DIR).";
    const ADB_GUIDANCE: &str =
        "Install Android Platform Tools (`adb`) from https://developer.android.com/tools/adb.";

    async fn probe_playwright() -> (Option<PathBuf>, bool, BackendReport) {
        let mut detected = Vec::new();
        let mut missing = Vec::new();
        let mut node_found = None;
        let mut module_found = false;

        if let Some(candidates) = WebBackend::candidates() {
            for node in candidates.nodes {
                let works = node.as_os_str() == NODE_TOOL
                    && Runtime::status(WebBackend::node_probe(&node))
                        .await
                        .is_ok_and(|status| status.success());
                if works {
                    node_found = Runtime::resolve_tool(Tool(NODE_TOOL)).await.or(Some(node));
                    break;
                } else if Runtime::is_file(&node).await {
                    node_found = Some(node);
                    break;
                }
            }
            for module in candidates.modules {
                if Runtime::exists(&module).await {
                    module_found = true;
                    detected.push(module);
                    break;
                }
            }
        }
        if let Some(path) = node_found.clone() {
            detected.insert(0, path);
        } else {
            missing.push(NODE_TOOL.to_owned());
        }
        if !module_found {
            missing.push(PLAYWRIGHT_DEP.to_owned());
        }

        let available = missing.is_empty();
        let report = BackendReport {
            id: "web-playwright".to_owned(),
            label: "Playwright (Web)".to_owned(),
            kind: BackendKind::Web,
            available,
            capabilities: Capabilities::of(&[
                Capability::Actions,
                Capability::UiTree,
                Capability::Video,
                Capability::ColorScheme,
                Capability::Scale,
                Capability::FullPage,
            ]),
            detected_binaries: detected,
            missing_dependencies: missing,
            install_guidance: PLAYWRIGHT_GUIDANCE.to_owned(),
        };
        (node_found, module_found, report)
    }

    async fn probe_cdp() -> BackendReport {
        let mut detected = Vec::new();
        for candidate in BROWSER_CANDIDATES {
            if let Some(path) = Runtime::resolve_tool(Tool(candidate)).await {
                detected.push(path);
            }
        }
        let available = !detected.is_empty();
        let missing = if available {
            Vec::new()
        } else {
            vec![CHROMIUM_DEP.to_owned()]
        };
        #[cfg(unix)]
        let (id, capabilities) = (
            "web-chromium-cdp",
            Capabilities::of(&[
                Capability::Actions,
                Capability::UiTree,
                Capability::ColorScheme,
                Capability::Scale,
                Capability::FullPage,
            ]),
        );
        #[cfg(not(unix))]
        let (id, capabilities) = ("web-chromium-headless", Capabilities::default());

        BackendReport {
            id: id.to_owned(),
            label: "Chromium CDP (Web)".to_owned(),
            kind: BackendKind::Web,
            available,
            capabilities,
            detected_binaries: detected,
            missing_dependencies: missing,
            install_guidance: CHROMIUM_GUIDANCE.to_owned(),
        }
    }

    #[cfg(unix)]
    async fn probe_x11() -> BackendReport {
        let mut detected = Vec::new();
        let mut missing = Vec::new();
        for tool in [XVFB_TOOL, FFMPEG_TOOL, PYTHON3_TOOL] {
            if let Some(path) = Runtime::resolve_tool(Tool(tool)).await {
                detected.push(path);
            } else {
                missing.push(tool.to_owned());
            }
        }
        BackendReport {
            id: "linux-x11-xvfb".to_owned(),
            label: "Linux X11 (Xvfb + ffmpeg)".to_owned(),
            kind: BackendKind::Desktop,
            available: missing.is_empty(),
            capabilities: Capabilities::of(&[Capability::Video, Capability::Pointer]),
            detected_binaries: detected,
            missing_dependencies: missing,
            install_guidance: X11_GUIDANCE.to_owned(),
        }
    }

    #[cfg(unix)]
    async fn probe_wayland() -> BackendReport {
        let mut detected = Vec::new();
        let mut missing = Vec::new();
        for tool in [XVFB_TOOL, FFMPEG_TOOL, PYTHON3_TOOL, DBUS_DAEMON_TOOL] {
            if let Some(path) = Runtime::resolve_tool(Tool(tool)).await {
                detected.push(path);
            } else {
                missing.push(tool.to_owned());
            }
        }
        let mut compositor_found = false;
        for compositor in WAYLAND_COMPOSITORS {
            if let Some(path) = Runtime::resolve_tool(Tool(compositor)).await {
                detected.push(path);
                compositor_found = true;
            }
        }
        if !compositor_found {
            missing.push("cosmic-comp | weston".to_owned());
        }
        BackendReport {
            id: "linux-wayland-nested".to_owned(),
            label: "Linux Wayland (Nested Compositor)".to_owned(),
            kind: BackendKind::Desktop,
            available: missing.is_empty(),
            capabilities: Capabilities::of(&[Capability::Video, Capability::Pointer]),
            detected_binaries: detected,
            missing_dependencies: missing,
            install_guidance: WAYLAND_GUIDANCE.to_owned(),
        }
    }

    #[cfg(unix)]
    async fn probe_broadway(
        node_found: Option<PathBuf>,
        playwright_module_found: bool,
    ) -> BackendReport {
        let mut detected = Vec::new();
        let mut missing = Vec::new();
        if let Some(path) = Runtime::resolve_tool(Tool(BROADWAY_DAEMON)).await {
            detected.push(path);
        } else {
            missing.push(BROADWAY_DAEMON.to_owned());
        }
        if let Some(path) = node_found {
            detected.push(path);
        } else {
            missing.push(NODE_TOOL.to_owned());
        }
        if !playwright_module_found {
            missing.push(PLAYWRIGHT_DEP.to_owned());
        }
        BackendReport {
            id: "linux-gtk4-broadway".to_owned(),
            label: "GTK4 Broadway (Linux)".to_owned(),
            kind: BackendKind::Desktop,
            available: missing.is_empty(),
            capabilities: Capabilities::of(&[Capability::Pointer, Capability::Video]),
            detected_binaries: detected,
            missing_dependencies: missing,
            install_guidance: BROADWAY_GUIDANCE.to_owned(),
        }
    }

    async fn probe_android() -> BackendReport {
        let mut detected = Vec::new();
        let mut missing = Vec::new();
        if let Some(path) = Runtime::resolve_tool(Tool(ADB)).await {
            detected.push(path);
        } else {
            missing.push(ADB.to_owned());
        }
        let _ = FFMPEG_TOOL;
        BackendReport {
            id: "mobile-android-adb".to_owned(),
            label: "Android (ADB)".to_owned(),
            kind: BackendKind::Mobile,
            available: missing.is_empty(),
            capabilities: Capabilities::of(&[Capability::Video, Capability::UiTree]),
            detected_binaries: detected,
            missing_dependencies: missing,
            install_guidance: ADB_GUIDANCE.to_owned(),
        }
    }

    /// Probes the host OS for every supported capture backend, reporting detected binaries,
    /// missing dependencies, capabilities, and OS installation guidance.
    #[must_use]
    pub async fn probe_environment() -> Vec<BackendReport> {
        let (node_found, module_found, playwright) = probe_playwright().await;
        let cdp = probe_cdp().await;
        let mut reports = vec![playwright, cdp];

        #[cfg(unix)]
        {
            reports.push(probe_x11().await);
            reports.push(probe_wayland().await);
            reports.push(probe_broadway(node_found, module_found).await);
        }
        #[cfg(not(unix))]
        {
            let _ = (node_found, module_found);
        }

        reports.push(probe_android().await);
        reports
    }
}
