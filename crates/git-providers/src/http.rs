//! Tiny shared HTTP client used by the GitHub and Azure REST shims.
//! Built on `reqwest` with rustls so the crate stays portable.

use std::sync::OnceLock;

use crate::ProviderError;

static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

pub fn client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(format!("senda/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("build reqwest client")
    })
}

pub fn map_reqwest(err: reqwest::Error) -> ProviderError {
    ProviderError::Other(err.into())
}
