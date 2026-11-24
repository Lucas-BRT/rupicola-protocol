//! The session contract, written once and generated for each flavour: a blocking pair in
//! `sync` and an async pair in `asynchronous`.

#[maybe_async_cfg::maybe(
    idents(flavour(sync = "sync", async = "asynchronous")),
    sync(feature = "sync"),
    async(feature = "async")
)]
pub(crate) mod flavour {
    use crate::session::{
        Capabilities, CaptureError, Checkpoint, Clip, Environment, Target, WaitCondition,
    };
    use rupicola_protocol::UiAction;
    use std::path::Path;

    /// A running capture of one target.
    #[cfg_attr(feature = "async", async_trait::async_trait)]
    pub trait CaptureSession: Send {
        /// # Errors
        ///
        /// Fails when the target does not accept the action.
        async fn perform(&mut self, action: &UiAction) -> Result<(), CaptureError>;

        /// # Errors
        ///
        /// Fails when the condition is not met in time.
        async fn wait_for(&mut self, condition: &WaitCondition) -> Result<(), CaptureError>;

        /// # Errors
        ///
        /// Fails when the frame cannot be grabbed.
        async fn screenshot(&mut self, id: &str) -> Result<Checkpoint, CaptureError>;

        /// Like [`Self::screenshot`], of the whole scrollable page. Only backends with
        /// [`crate::session::Capability::FullPage`] override it.
        ///
        /// # Errors
        ///
        /// Fails when the backend cannot shoot more than what it shows, or the frame cannot be
        /// grabbed.
        async fn screenshot_full_page(&mut self, _id: &str) -> Result<Checkpoint, CaptureError> {
            Err(CaptureError::environment(
                "this backend cannot take a full-page checkpoint",
            ))
        }

        /// # Errors
        ///
        /// Fails when recording cannot start.
        async fn start_recording(&mut self) -> Result<(), CaptureError>;

        /// # Errors
        ///
        /// Fails when no recording is running or it cannot be finished.
        async fn stop_recording(&mut self, id: &str) -> Result<Clip, CaptureError>;

        /// Stops the target and everything started for it. Called exactly once, even after
        /// a failed step.
        ///
        /// # Errors
        ///
        /// Fails when cleanup does not complete.
        async fn finish(self: Box<Self>) -> Result<(), CaptureError>;
    }

    /// A way of capturing: knows what it can do and how to start a [`CaptureSession`].
    #[cfg_attr(feature = "async", async_trait::async_trait)]
    pub trait CaptureBackend: Send + Sync {
        fn id(&self) -> &'static str;

        fn capabilities(&self) -> Capabilities;

        /// Launches `target` in `environment`; files go under `out`.
        ///
        /// # Errors
        ///
        /// [`crate::session::ErrorClass::Environment`] when tools are missing,
        /// [`crate::session::ErrorClass::Target`] when the target does not come up.
        async fn start(
            &self,
            target: &Target,
            environment: &Environment,
            out: &Path,
        ) -> Result<Box<dyn CaptureSession>, CaptureError>;
    }
}
