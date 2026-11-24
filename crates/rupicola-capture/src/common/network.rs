//! Shared `NetworkPolicy` enforcement (syntax + DNS resolution), generated for both
//! `sync` and `asynchronous` flavours.

#[maybe_async_cfg::maybe(
    idents(
        flavour(sync = "sync", async = "asynchronous"),
        rt(sync = "rt_sync", async = "rt_async")
    ),
    sync(feature = "sync"),
    async(feature = "async")
)]
pub(crate) mod flavour {
    use crate::common::rt::Runtime;
    use crate::session::{CaptureError, NetworkPolicy};

    /// Checks `url` against `policy`, resolving DNS if `url` uses a hostname.
    pub(crate) async fn check_network_url(
        policy: NetworkPolicy,
        url: &str,
    ) -> Result<(), CaptureError> {
        let Some((host, port)) = policy.check_url_syntax(url)? else {
            return Ok(());
        };
        let ips = Runtime::lookup_ips(&host, port).await?;
        if ips.is_empty() {
            return Err(CaptureError::target(format!(
                "capture host `{host}` resolved to no addresses"
            )));
        }
        for ip in ips {
            if !NetworkPolicy::is_public_ip(ip) {
                return Err(CaptureError::target(format!(
                    "capture host `{host}` resolved to non-public address `{ip}`"
                )));
            }
        }
        Ok(())
    }
}
