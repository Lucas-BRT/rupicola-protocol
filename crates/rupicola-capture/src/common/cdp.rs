//! The Chromium backend as a session, written once and generated for each flavour.
//!
//! On Unix the session keeps one headless browser and speaks the Chrome `DevTools` Protocol to
//! it over a pair of pipes: JSON messages, each ended by a NUL byte. Elsewhere the browser is
//! run again for each screenshot.

#[maybe_async_cfg::maybe(
    idents(
        flavour(sync = "sync", async = "asynchronous"),
        rt(sync = "rt_sync", async = "rt_async")
    ),
    sync(feature = "sync"),
    async(feature = "async")
)]
pub(crate) mod flavour {
    use crate::backend::CdpBackend;
    use crate::backend::artifacts::Artifacts;
    use crate::backend::cdp::BROWSER_CANDIDATES;
    #[cfg(unix)]
    use crate::backend::cdp::PAGE_HELPERS_JS;
    #[cfg(unix)]
    use crate::backend::dom_tree::{DOM_SNAPSHOT_JS, DomTree};
    use crate::backend::tool::Tool;
    #[cfg(unix)]
    use crate::common::rt::Child;
    use crate::common::rt::Runtime;
    use crate::common::traits::flavour::{CaptureBackend, CaptureSession};
    use crate::session::{
        Capabilities, CaptureError, Checkpoint, Clip, Environment, NetworkPolicy, Target,
        WaitCondition,
    };
    #[cfg(unix)]
    use crate::session::{Capability, ColorScheme, UiTreeMode};
    use rupicola_protocol::UiAction;
    #[cfg(unix)]
    use rupicola_protocol::UiSnapshot;
    #[cfg(unix)]
    use serde_json::{Value, json};
    use std::path::{Path, PathBuf};
    #[cfg(unix)]
    use std::time::{Duration, Instant};

    /// The byte that ends every message on the protocol's pipes.
    #[cfg(unix)]
    const MESSAGE_END: u8 = 0;
    /// Folder, inside the output directory, that holds the browser's profile for a session.
    #[cfg(unix)]
    const PROFILE_DIR: &str = ".chromium-profile";
    /// Milliseconds between two looks at a page that is being waited for.
    #[cfg(unix)]
    const POLL_MS: u64 = 100;
    #[cfg(unix)]
    const SETTLE_STABLE_FRAMES: u32 = 3;
    #[cfg(unix)]
    const SETTLE_LIMIT_MS: u64 = 600;
    /// Milliseconds a step may take when the plan names no limit.
    #[cfg(unix)]
    const DEFAULT_TIMEOUT_MS: u64 = 10_000;
    /// The browser's helper processes keep writing to its profile for a moment after it is
    /// stopped, so removing the profile is tried again this many times, this far apart.
    #[cfg(unix)]
    const PROFILE_REMOVE_ATTEMPTS: u32 = 20;
    #[cfg(unix)]
    const PROFILE_REMOVE_RETRY_MS: u64 = 100;

    /// Finds a Chromium-family browser.
    #[cfg_attr(feature = "async", async_trait::async_trait)]
    pub trait CdpDiscovery: Sized {
        /// Discovers a Chromium-family browser binary in `PATH`.
        async fn discover() -> Option<Self>;
    }

    #[cfg_attr(feature = "async", async_trait::async_trait)]
    impl CdpDiscovery for CdpBackend {
        async fn discover() -> Option<Self> {
            for candidate in BROWSER_CANDIDATES {
                if Runtime::is_available(Tool(candidate)).await {
                    return Some(Self::new(candidate));
                }
            }
            None
        }
    }

    #[cfg(unix)]
    #[cfg_attr(feature = "async", async_trait::async_trait)]
    impl CaptureBackend for CdpBackend {
        fn id(&self) -> &'static str {
            "web-chromium-cdp"
        }

        fn capabilities(&self) -> Capabilities {
            Capabilities::of(&[
                Capability::Actions,
                Capability::UiTree,
                Capability::ColorScheme,
                Capability::Scale,
                Capability::FullPage,
            ])
        }

        async fn start(
            &self,
            target: &Target,
            environment: &Environment,
            out: &Path,
        ) -> Result<Box<dyn CaptureSession>, CaptureError> {
            crate::common::network::flavour::check_network_url(
                environment.network_policy,
                &target.command,
            )
            .await?;
            Runtime::create_dir_all(out)
                .await
                .map_err(|e| CaptureError::environment(format!("create {}: {e}", out.display())))?;
            let profile = out.join(PROFILE_DIR);
            let _ = Runtime::remove_dir_all(&profile).await;
            let child = Runtime::spawn(self.launch(environment, &profile)).map_err(|e| {
                CaptureError::environment(format!("run {}: {e}", self.browser_bin.display()))
            })?;
            if !child.has_pipes() {
                return Err(CaptureError::environment(
                    "the browser has no protocol pipes",
                ));
            }
            let mut session = CdpSession {
                child,
                session_id: String::new(),
                next_id: 0,
                artifacts: Artifacts::new(out),
                profile,
                tree: environment.ui_tree != UiTreeMode::Off,
                timeout_ms: environment.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS),
                settle_ms: environment.settle_ms.unwrap_or(0),
                network_policy: environment.network_policy,
            };
            if let Err(error) = session.open(&target.command, environment).await {
                let _ = Box::new(session).finish().await;
                return Err(error);
            }
            Ok(Box::new(session))
        }
    }

    /// One page of a headless browser, driven over the Chrome `DevTools` Protocol.
    #[cfg(unix)]
    struct CdpSession {
        child: Child,
        /// The page's session on the protocol, once it is attached.
        session_id: String,
        /// The id of the last command sent.
        next_id: u64,
        artifacts: Artifacts,
        profile: PathBuf,
        /// Whether the plan wants the page's tree with each screenshot.
        tree: bool,
        timeout_ms: u64,
        settle_ms: u64,
        network_policy: NetworkPolicy,
    }

    #[cfg(unix)]
    impl CdpSession {
        /// Sends a CDP command without waiting for its reply, returning its assigned id.
        async fn send_only(
            &mut self,
            to_page: bool,
            method: &str,
            params: Value,
        ) -> Result<u64, CaptureError> {
            self.next_id += 1;
            let id = self.next_id;
            let mut message = json!({ "id": id, "method": method });
            message["params"] = params;
            if to_page {
                message["sessionId"] = Value::String(self.session_id.clone());
            }
            let mut bytes = serde_json::to_vec(&message)
                .map_err(|e| CaptureError::environment(format!("write to the browser: {e}")))?;
            bytes.push(MESSAGE_END);
            self.child
                .write(&bytes)
                .await
                .map_err(|e| CaptureError::environment(format!("write to the browser: {e}")))?;
            Ok(id)
        }

        /// Sends a command and waits for its reply. Events, and replies to anything else,
        /// are read past. `to_page` aims the command at the page instead of the browser.
        async fn request(
            &mut self,
            to_page: bool,
            method: &str,
            params: Value,
        ) -> Result<Value, CaptureError> {
            let id = self.send_only(to_page, method, params).await?;
            loop {
                let raw = self
                    .child
                    .read_message(MESSAGE_END)
                    .await
                    .map_err(|e| CaptureError::environment(format!("read from the browser: {e}")))?
                    .ok_or_else(|| CaptureError::environment("the browser exited"))?;
                let reply: Value = serde_json::from_slice(&raw).map_err(|e| {
                    CaptureError::environment(format!("read from the browser: {e}"))
                })?;
                if self.network_policy == NetworkPolicy::PublicOnly
                    && reply.get("method").and_then(Value::as_str) == Some("Fetch.requestPaused")
                    && let Some(req_id) = reply.pointer("/params/requestId").and_then(Value::as_str)
                {
                    let req_url = reply
                        .pointer("/params/request/url")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let allowed = crate::common::network::flavour::check_network_url(
                        NetworkPolicy::PublicOnly,
                        req_url,
                    )
                    .await
                    .is_ok();
                    if allowed {
                        let _ = self
                            .send_only(
                                true,
                                "Fetch.continueRequest",
                                json!({ "requestId": req_id }),
                            )
                            .await;
                    } else {
                        let _ = self
                            .send_only(
                                true,
                                "Fetch.failRequest",
                                json!({ "requestId": req_id, "errorReason": "BlockedByClient" }),
                            )
                            .await;
                    }
                    continue;
                }
                if reply.get("id").and_then(Value::as_u64) != Some(id) {
                    continue;
                }
                if let Some(error) = reply.get("error") {
                    let said = error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("no reason given");
                    return Err(CaptureError::target(format!("{method} failed: {said}")));
                }
                return Ok(reply.get("result").cloned().unwrap_or(Value::Null));
            }
        }

        /// Runs `expression` in the page and returns its value, optionally awaiting promises.
        async fn evaluate_raw(
            &mut self,
            expression: &str,
            await_promise: bool,
        ) -> Result<Value, CaptureError> {
            let result = self
                .request(
                    true,
                    "Runtime.evaluate",
                    json!({
                        "expression": expression,
                        "returnByValue": true,
                        "awaitPromise": await_promise
                    }),
                )
                .await?;
            if let Some(exception) = result.get("exceptionDetails") {
                let text = exception
                    .pointer("/exception/description")
                    .or_else(|| exception.get("text"))
                    .and_then(Value::as_str)
                    .unwrap_or("the script failed");
                return Err(CaptureError::target(format!(
                    "the page script failed: {text}"
                )));
            }
            Ok(result
                .pointer("/result/value")
                .cloned()
                .unwrap_or(Value::Null))
        }

        /// Runs `expression` in the page and returns its value.
        async fn evaluate(&mut self, expression: &str) -> Result<Value, CaptureError> {
            self.evaluate_raw(expression, false).await
        }

        async fn helper(&mut self, call: &str) -> Result<Value, CaptureError> {
            self.evaluate(&format!("{PAGE_HELPERS_JS}\n{call}")).await
        }

        async fn settle(&mut self) -> Result<(), CaptureError> {
            let expr =
                format!("{PAGE_HELPERS_JS}\ncdpSettle({SETTLE_STABLE_FRAMES}, {SETTLE_LIMIT_MS})");
            self.evaluate_raw(&expr, true).await?;
            if self.settle_ms > 0 {
                Runtime::sleep(self.settle_ms).await;
            }
            Ok(())
        }

        async fn target_center(&mut self, target_id: &str) -> Result<(f64, f64), CaptureError> {
            let arg = serde_json::to_string(target_id)
                .map_err(|e| CaptureError::environment(e.to_string()))?;
            let val = self.helper(&format!("cdpCenter({arg})")).await?;
            let x = val.get("x").and_then(Value::as_f64);
            let y = val.get("y").and_then(Value::as_f64);
            match (x, y) {
                (Some(x), Some(y)) => Ok((x, y)),
                _ => Err(CaptureError::target(format!(
                    "missing coordinates for {target_id}"
                ))),
            }
        }

        async fn mouse_click(&mut self, x: f64, y: f64) -> Result<(), CaptureError> {
            self.request(
                true,
                "Input.dispatchMouseEvent",
                json!({ "type": "mouseMoved", "x": x, "y": y }),
            )
            .await?;
            self.request(
                true,
                "Input.dispatchMouseEvent",
                json!({
                    "type": "mousePressed",
                    "x": x,
                    "y": y,
                    "button": "left",
                    "buttons": 1,
                    "clickCount": 1
                }),
            )
            .await?;
            self.request(
                true,
                "Input.dispatchMouseEvent",
                json!({
                    "type": "mouseReleased",
                    "x": x,
                    "y": y,
                    "button": "left",
                    "clickCount": 1
                }),
            )
            .await?;
            Ok(())
        }

        fn modifier_mask(modifiers: &[String]) -> u32 {
            let mut mask = 0_u32;
            for m in modifiers {
                match m.to_ascii_lowercase().as_str() {
                    "alt" => mask |= 1,
                    "control" | "ctrl" => mask |= 2,
                    "meta" | "command" | "cmd" => mask |= 4,
                    "shift" => mask |= 8,
                    _ => {}
                }
            }
            mask
        }

        async fn save_screenshot(
            &mut self,
            id: &str,
            full_page: bool,
        ) -> Result<Checkpoint, CaptureError> {
            let png = self.artifacts.png(id)?;
            let params = if full_page {
                json!({ "format": "png", "captureBeyondViewport": true })
            } else {
                json!({ "format": "png" })
            };
            let shot = self.request(true, "Page.captureScreenshot", params).await?;
            let data = shot
                .get("data")
                .and_then(Value::as_str)
                .and_then(crate::backend::base64::decode)
                .filter(|bytes| !bytes.is_empty())
                .ok_or_else(|| CaptureError::target("the browser sent no screenshot"))?;
            Runtime::write(&png, &data)
                .await
                .map_err(|e| CaptureError::environment(format!("write {}: {e}", png.display())))?;
            let snapshot = if self.tree {
                Some(self.page_tree().await?)
            } else {
                None
            };
            Ok(Checkpoint {
                id: id.to_owned(),
                png,
                snapshot,
            })
        }

        /// Looks at the page until `condition`, a script that answers true, holds.
        async fn until(&mut self, condition: &str, what: &str) -> Result<(), CaptureError> {
            let expression = format!("{PAGE_HELPERS_JS}\n{condition}");
            let deadline = Instant::now() + Duration::from_millis(self.timeout_ms);
            loop {
                if self.evaluate(&expression).await? == Value::Bool(true) {
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    return Err(CaptureError::target(format!(
                        "timed out after {} ms waiting for {what}",
                        self.timeout_ms
                    )));
                }
                Runtime::sleep(POLL_MS).await;
            }
        }

        /// Attaches to a page, shows it as `environment` says, and loads `url` into it.
        async fn open(&mut self, url: &str, environment: &Environment) -> Result<(), CaptureError> {
            let created = self
                .request(
                    false,
                    "Target.createTarget",
                    json!({ "url": "about:blank" }),
                )
                .await?;
            let target = Self::text(&created, "targetId")?;
            let attached = self
                .request(
                    false,
                    "Target.attachToTarget",
                    json!({ "targetId": target, "flatten": true }),
                )
                .await?;
            self.session_id = Self::text(&attached, "sessionId")?;
            let [width, height] = environment.viewport;
            self.request(
                true,
                "Emulation.setDeviceMetricsOverride",
                json!({
                    "width": width,
                    "height": height,
                    "deviceScaleFactor": environment.scale,
                    "mobile": false,
                }),
            )
            .await?;
            let mut features = Vec::new();
            if let Some(scheme) = environment.color_scheme {
                let value = match scheme {
                    ColorScheme::Light => "light",
                    ColorScheme::Dark => "dark",
                };
                features.push(json!({ "name": "prefers-color-scheme", "value": value }));
            }
            if environment.reduced_motion {
                features.push(json!({ "name": "prefers-reduced-motion", "value": "reduce" }));
            }
            if !features.is_empty() {
                self.request(
                    true,
                    "Emulation.setEmulatedMedia",
                    json!({ "features": features }),
                )
                .await?;
            }
            if let Some(locale) = &environment.locale {
                // Some browsers refuse the override; a page without it still loads.
                let _ = self
                    .request(
                        true,
                        "Emulation.setLocaleOverride",
                        json!({ "locale": locale }),
                    )
                    .await;
            }
            if self.network_policy == NetworkPolicy::PublicOnly {
                self.request(
                    true,
                    "Fetch.enable",
                    json!({ "patterns": [{ "urlPattern": "*" }] }),
                )
                .await?;
            }
            let navigated = self
                .request(true, "Page.navigate", json!({ "url": url }))
                .await?;
            if let Some(why) = navigated.get("errorText").and_then(Value::as_str) {
                return Err(CaptureError::target(format!("{url} did not load: {why}")));
            }
            self.until("cdpLoaded()", "the page to load").await?;
            let _ = self.request(true, "Page.bringToFront", json!({})).await;
            let _ = self
                .request(
                    true,
                    "Emulation.setFocusEmulationEnabled",
                    json!({ "enabled": true }),
                )
                .await;
            Runtime::sleep(environment.warmup_ms).await;
            Ok(())
        }

        fn text(value: &Value, key: &str) -> Result<String, CaptureError> {
            value
                .get(key)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| CaptureError::environment(format!("the browser sent no {key}")))
        }

        /// The tree of the page as it is now.
        async fn page_tree(&mut self) -> Result<UiSnapshot, CaptureError> {
            let value = self
                .evaluate(&format!("{DOM_SNAPSHOT_JS}\ndomSnapshot()"))
                .await?;
            let tree: UiSnapshot = serde_json::from_value(value)
                .map_err(|e| CaptureError::target(format!("the page's tree is malformed: {e}")))?;
            Ok(DomTree::finish(tree))
        }

        fn unsupported() -> CaptureError {
            CaptureError::environment(
                "this backend cannot act on the page by element or record it: only wait-ms is supported",
            )
        }
    }

    #[cfg(unix)]
    #[cfg_attr(feature = "async", async_trait::async_trait)]
    impl CaptureSession for CdpSession {
        async fn perform(&mut self, action: &UiAction) -> Result<(), CaptureError> {
            match action {
                UiAction::WaitMs { ms } => {
                    Runtime::sleep(*ms).await;
                    return Ok(());
                }
                UiAction::Click { target_id } => {
                    let (x, y) = self.target_center(target_id).await?;
                    self.mouse_click(x, y).await?;
                }
                UiAction::ClickAt { x, y } => {
                    self.mouse_click(*x, *y).await?;
                }
                UiAction::Hover { target_id } => {
                    let (x, y) = self.target_center(target_id).await?;
                    self.request(
                        true,
                        "Input.dispatchMouseEvent",
                        json!({ "type": "mouseMoved", "x": x, "y": y }),
                    )
                    .await?;
                }
                UiAction::Focus { target_id } => {
                    let arg = serde_json::to_string(target_id)
                        .map_err(|e| CaptureError::environment(e.to_string()))?;
                    self.helper(&format!("cdpFocus({arg}, false)")).await?;
                }
                UiAction::TypeText { target_id, text } => {
                    let arg = serde_json::to_string(target_id)
                        .map_err(|e| CaptureError::environment(e.to_string()))?;
                    self.helper(&format!("cdpFocus({arg}, true)")).await?;
                    self.request(true, "Input.insertText", json!({ "text": text }))
                        .await?;
                }
                UiAction::PressKey { key, modifiers } => {
                    let mods = Self::modifier_mask(modifiers);
                    self.request(
                        true,
                        "Input.dispatchKeyEvent",
                        json!({ "type": "keyDown", "key": key, "modifiers": mods }),
                    )
                    .await?;
                    self.request(
                        true,
                        "Input.dispatchKeyEvent",
                        json!({ "type": "keyUp", "key": key, "modifiers": mods }),
                    )
                    .await?;
                }
                UiAction::Scroll { target_id, dx, dy } => {
                    if let Some(target_id) = target_id {
                        let arg = serde_json::to_string(target_id)
                            .map_err(|e| CaptureError::environment(e.to_string()))?;
                        self.helper(&format!("cdpScroll({arg}, {dx}, {dy})"))
                            .await?;
                    } else {
                        self.request(
                            true,
                            "Input.dispatchMouseEvent",
                            json!({
                                "type": "mouseWheel",
                                "x": 1.0,
                                "y": 1.0,
                                "deltaX": dx,
                                "deltaY": dy
                            }),
                        )
                        .await?;
                    }
                }
                UiAction::ScrollIntoView { target_id } => {
                    let arg = serde_json::to_string(target_id)
                        .map_err(|e| CaptureError::environment(e.to_string()))?;
                    self.helper(&format!("cdpScrollIntoView({arg})")).await?;
                }
                UiAction::ScrollTo { target_id, x, y } => {
                    let arg = target_id
                        .as_deref()
                        .map(serde_json::to_string)
                        .transpose()
                        .map_err(|e| CaptureError::environment(e.to_string()))?
                        .unwrap_or_else(|| "null".to_owned());
                    self.helper(&format!("cdpScrollTo({arg}, {x}, {y})"))
                        .await?;
                }
                UiAction::ScrollToEnd { target_id } => {
                    let arg = target_id
                        .as_deref()
                        .map(serde_json::to_string)
                        .transpose()
                        .map_err(|e| CaptureError::environment(e.to_string()))?
                        .unwrap_or_else(|| "null".to_owned());
                    self.helper(&format!("cdpScrollTo({arg}, null, null)"))
                        .await?;
                }
            }
            self.settle().await
        }

        async fn wait_for(&mut self, condition: &WaitCondition) -> Result<(), CaptureError> {
            match condition {
                WaitCondition::Delay { ms } => {
                    Runtime::sleep(*ms).await;
                    Ok(())
                }
                WaitCondition::Visible { target_id } => {
                    let target = serde_json::to_string(target_id)
                        .map_err(|e| CaptureError::environment(e.to_string()))?;
                    self.until(
                        &format!("cdpVisible({target})"),
                        &format!("{target_id} to show"),
                    )
                    .await
                }
                WaitCondition::Text { text } => {
                    let wanted = serde_json::to_string(text)
                        .map_err(|e| CaptureError::environment(e.to_string()))?;
                    self.until(
                        &format!("cdpHasText({wanted})"),
                        &format!("the text {text}"),
                    )
                    .await
                }
            }
        }

        async fn screenshot(&mut self, id: &str) -> Result<Checkpoint, CaptureError> {
            self.save_screenshot(id, false).await
        }

        async fn screenshot_full_page(&mut self, id: &str) -> Result<Checkpoint, CaptureError> {
            self.save_screenshot(id, true).await
        }

        async fn start_recording(&mut self) -> Result<(), CaptureError> {
            Err(Self::unsupported())
        }

        async fn stop_recording(&mut self, _id: &str) -> Result<Clip, CaptureError> {
            Err(Self::unsupported())
        }

        async fn finish(mut self: Box<Self>) -> Result<(), CaptureError> {
            // The browser may be gone before it answers; there is nothing to do about that.
            let _ = self.request(false, "Browser.close", json!({})).await;
            self.child.kill().await;
            for attempt in 0..PROFILE_REMOVE_ATTEMPTS {
                if attempt > 0 {
                    Runtime::sleep(PROFILE_REMOVE_RETRY_MS).await;
                }
                let _ = Runtime::remove_dir_all(&self.profile).await;
                if !Runtime::exists(&self.profile).await {
                    break;
                }
            }
            Ok(())
        }
    }

    /// The browser, run again for each screenshot, where its protocol pipes do not exist.
    #[cfg(not(unix))]
    #[cfg_attr(feature = "async", async_trait::async_trait)]
    impl CaptureBackend for CdpBackend {
        fn id(&self) -> &'static str {
            "web-chromium-headless"
        }

        fn capabilities(&self) -> Capabilities {
            Capabilities::default()
        }

        async fn start(
            &self,
            target: &Target,
            environment: &Environment,
            out: &Path,
        ) -> Result<Box<dyn CaptureSession>, CaptureError> {
            crate::common::network::flavour::check_network_url(
                environment.network_policy,
                &target.command,
            )
            .await?;
            Runtime::create_dir_all(out)
                .await
                .map_err(|e| CaptureError::environment(format!("create {}: {e}", out.display())))?;
            Ok(Box::new(OneShotSession {
                backend: self.clone(),
                url: target.command.clone(),
                environment: environment.clone(),
                artifacts: Artifacts::new(out),
            }))
        }
    }

    /// A page captured by one headless browser run per screenshot.
    #[cfg(not(unix))]
    struct OneShotSession {
        backend: CdpBackend,
        url: String,
        environment: Environment,
        artifacts: Artifacts,
    }

    #[cfg(not(unix))]
    impl OneShotSession {
        fn unsupported(what: &str) -> CaptureError {
            CaptureError::environment(format!(
                "this backend cannot {what}: it only takes screenshots"
            ))
        }
    }

    #[cfg(not(unix))]
    #[cfg_attr(feature = "async", async_trait::async_trait)]
    impl CaptureSession for OneShotSession {
        async fn perform(&mut self, action: &UiAction) -> Result<(), CaptureError> {
            match action {
                UiAction::WaitMs { ms } => {
                    Runtime::sleep(*ms).await;
                    Ok(())
                }
                _ => Err(Self::unsupported("act on the page")),
            }
        }

        async fn wait_for(&mut self, condition: &WaitCondition) -> Result<(), CaptureError> {
            match condition {
                WaitCondition::Delay { ms } => {
                    Runtime::sleep(*ms).await;
                    Ok(())
                }
                WaitCondition::Visible { .. } | WaitCondition::Text { .. } => {
                    Err(Self::unsupported("wait on the page"))
                }
            }
        }

        async fn screenshot(&mut self, id: &str) -> Result<Checkpoint, CaptureError> {
            let png = self.artifacts.png(id)?;
            let status =
                Runtime::status(self.backend.screenshot(&self.url, &self.environment, &png))
                    .await
                    .map_err(|e| {
                        CaptureError::environment(format!(
                            "run {}: {e}",
                            self.backend.browser_bin.display()
                        ))
                    })?;
            if !status.success() || !Runtime::exists(&png).await {
                return Err(CaptureError::target(format!(
                    "headless browser screenshot failed with {status}"
                )));
            }
            Ok(Checkpoint {
                id: id.to_owned(),
                png,
                snapshot: None,
            })
        }

        async fn start_recording(&mut self) -> Result<(), CaptureError> {
            Err(Self::unsupported("record"))
        }

        async fn stop_recording(&mut self, _id: &str) -> Result<Clip, CaptureError> {
            Err(Self::unsupported("record"))
        }

        async fn finish(self: Box<Self>) -> Result<(), CaptureError> {
            Ok(())
        }
    }
}
