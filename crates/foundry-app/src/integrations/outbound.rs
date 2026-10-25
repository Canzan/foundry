//! `OutboundHttp`: the one HTTP client for addresses an admin or a lead
//! supplies (DDR-14, ADR-EIP-006 decision 6). Every address it connects to
//! has passed the outbound address policy ([`foundry_integrations::judge`]):
//!
//! - names are resolved by a judging resolver that hands `reqwest` only the
//!   approved addresses, at connect time, so a name that rebinds to a
//!   refused address between checks cannot be reached;
//! - address literals (in any spelling) are judged before the request leaves;
//! - it follows no redirect (a redirect is an error, not a request) and uses
//!   no proxy (a proxy would resolve the target itself, past the judge).
//!
//! The Slack client (a fixed, operator-set host) is separate. check-arch
//! `integrations-outbound-client-seam` keeps every other client out of this
//! module. Loopback is reachable only through [`outbound_seam`], which exists
//! only in a `test-support` build (DEVOPS U-D1, C-7).

use foundry_integrations::{judge_host, OutboundAllow, OutboundTarget, Verdict};
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const CALL_TIMEOUT: Duration = Duration::from_secs(10);

/// Why an outbound request did not get an answer to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum OutboundError {
    #[error("the address is refused by the outbound address policy")]
    Refused,
    #[error("the address answered with a redirect, which Foundry does not follow")]
    Redirected,
    #[error("the address could not be reached")]
    Unreachable,
}

/// The resolver's refusal, found again in the error `reqwest` reports.
#[derive(Debug, thiserror::Error)]
#[error("every address of the host is refused by the outbound address policy")]
struct AddressRefused;

type LookupFuture = Pin<Box<dyn Future<Output = std::io::Result<Vec<IpAddr>>> + Send>>;
type Lookup = Arc<dyn Fn(String) -> LookupFuture + Send + Sync>;

/// The system resolver.
fn system_lookup() -> Lookup {
    Arc::new(|host: String| {
        Box::pin(async move {
            let found = tokio::net::lookup_host((host.as_str(), 0)).await?;
            Ok(found.map(|addr| addr.ip()).collect())
        })
    })
}

/// Resolves a name and keeps only the addresses the judge allows.
struct JudgingResolver {
    allow: Arc<OutboundAllow>,
    lookup: Lookup,
}

impl Resolve for JudgingResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_string();
        let allow = self.allow.clone();
        let found = (self.lookup)(host.clone());
        Box::pin(async move {
            let approved: Vec<SocketAddr> = found
                .await?
                .into_iter()
                .filter(|ip| judge_host(&host, *ip, &allow) == Verdict::Allow)
                .map(|ip| SocketAddr::new(ip, 0))
                .collect();
            if approved.is_empty() {
                return Err(Box::new(AddressRefused) as Box<dyn std::error::Error + Send + Sync>);
            }
            Ok(Box::new(approved.into_iter()) as Addrs)
        })
    }
}

/// The client for user-supplied hosts.
#[derive(Clone)]
pub struct OutboundHttp {
    client: reqwest::Client,
    allow: Arc<OutboundAllow>,
}

impl OutboundHttp {
    /// The production client: the system resolver, judged.
    pub fn new(allow: OutboundAllow) -> Self {
        Self::build(allow, system_lookup(), |builder| builder)
    }

    fn build(
        allow: OutboundAllow,
        lookup: Lookup,
        extend: impl FnOnce(reqwest::ClientBuilder) -> reqwest::ClientBuilder,
    ) -> Self {
        let allow = Arc::new(allow);
        let builder = reqwest::Client::builder()
            .dns_resolver(Arc::new(JudgingResolver {
                allow: allow.clone(),
                lookup,
            }))
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(CALL_TIMEOUT);
        let client = extend(builder)
            .build()
            .expect("the outbound client builds (its TLS backend is compiled in)");
        Self { client, allow }
    }

    /// A request to `url`, sent with [`OutboundHttp::send`].
    pub fn request(&self, method: reqwest::Method, url: &str) -> reqwest::RequestBuilder {
        self.client.request(method, url)
    }

    /// Send `request` if its address passes the policy. A redirect answer is
    /// [`OutboundError::Redirected`]: it is never followed.
    pub async fn send(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, OutboundError> {
        let request = request.build().map_err(|_| OutboundError::Unreachable)?;
        let target =
            OutboundTarget::parse(request.url().as_str()).map_err(|_| OutboundError::Refused)?;
        if let Some(Verdict::Refuse(_)) = target.verdict(&self.allow) {
            return Err(OutboundError::Refused);
        }
        let response = self.client.execute(request).await.map_err(|error| {
            if refused_by_the_resolver(&error) {
                OutboundError::Refused
            } else {
                OutboundError::Unreachable
            }
        })?;
        if response.status().is_redirection() {
            return Err(OutboundError::Redirected);
        }
        Ok(response)
    }
}

fn refused_by_the_resolver(error: &reqwest::Error) -> bool {
    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(cause) = source {
        if cause.is::<AddressRefused>() {
            return true;
        }
        source = cause.source();
    }
    false
}

/// The test-support outbound seam (DEVOPS U-D1, C-7): an [`OutboundHttp`]
/// that also trusts a test CA and reaches one named test host at a loopback
/// socket, or resolves names through a test's own lookup. Production builds
/// cannot contain it (feature `test-support` is off in the image; check-arch
/// `integrations-test-seam`; the Dockerfile refuses a binary carrying the
/// marker).
#[cfg(feature = "test-support")]
pub mod outbound_seam {
    use super::{Lookup, OutboundHttp};
    use foundry_integrations::OutboundAllow;
    use std::net::{IpAddr, SocketAddr};
    use std::sync::Arc;

    /// The seam's unique marker: present in a binary only if the seam is.
    pub const OUTBOUND_SEAM_MARKER: &str = "foundry-test-seam:integrations-outbound-v1";

    /// Reach `host` at `socket` (a loopback test listener), trusting
    /// `root_pem` as well as the public roots. Every other host is judged.
    pub fn admitting(
        allow: OutboundAllow,
        host: &str,
        socket: SocketAddr,
        root_pem: Option<&[u8]>,
    ) -> OutboundHttp {
        tracing::debug!(
            seam = OUTBOUND_SEAM_MARKER,
            host,
            "integrations.outbound.admitted"
        );
        let root = root_pem.map(|pem| reqwest::Certificate::from_pem(pem).expect("a test CA"));
        OutboundHttp::build(allow, super::system_lookup(), |builder| {
            let builder = builder.resolve(host, socket);
            match root {
                Some(root) => builder.add_root_certificate(root),
                None => builder,
            }
        })
    }

    /// Resolve every name through `lookup` instead of the system resolver;
    /// what it answers is judged as a real answer would be.
    pub fn resolving(
        allow: OutboundAllow,
        lookup: impl Fn(&str) -> Vec<IpAddr> + Send + Sync + 'static,
    ) -> OutboundHttp {
        let lookup = Arc::new(lookup);
        let judged: Lookup = Arc::new(move |host: String| {
            let found = lookup(&host);
            Box::pin(async move { Ok(found) })
        });
        OutboundHttp::build(allow, judged, |builder| builder)
    }
}
