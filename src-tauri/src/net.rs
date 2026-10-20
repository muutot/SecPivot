//! TLS trust configuration shared by every `reqwest` client.
//!
//! `reqwest` 0.13's `rustls` feature selects `rustls-platform-verifier` when no
//! explicit roots are configured. On Android that verifier needs JNI plus a
//! Kotlin component that Tauri does not ship, so the first handshake panics
//! with "Expect rustls-platform-verifier to be initialized" and takes down the
//! client's event loop. Supplying an explicit Mozilla root bundle switches
//! `reqwest` to the webpki verifier and never touches the platform verifier.
//!
//! Consequence: user-installed enterprise CAs are not trusted on Android. That
//! is the deliberate trade-off for a client that can connect at all; revisit if
//! Android ever initializes the platform verifier for us.

/// Apply the Android trust overrides to an async client builder. Desktop and
/// iOS keep the platform verifier.
pub(crate) fn client_builder(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    #[cfg(target_os = "android")]
    {
        builder.tls_certs_only(android_roots())
    }
    #[cfg(not(target_os = "android"))]
    {
        builder
    }
}

/// Blocking-client counterpart of [`client_builder`] (S3 / WebDAV transports).
pub(crate) fn blocking_client_builder(
    builder: reqwest::blocking::ClientBuilder,
) -> reqwest::blocking::ClientBuilder {
    #[cfg(target_os = "android")]
    {
        builder.tls_certs_only(android_roots())
    }
    #[cfg(not(target_os = "android"))]
    {
        builder
    }
}

#[cfg(target_os = "android")]
fn android_roots() -> impl Iterator<Item = reqwest::Certificate> {
    webpki_root_certs::TLS_SERVER_ROOT_CERTS
        .iter()
        .filter_map(|der| reqwest::Certificate::from_der(der.as_ref()).ok())
}
