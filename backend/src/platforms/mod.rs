//! Transport boundaries: domain services do not depend on Discord payloads or IDs.
//! Future providers implement this contract and their own authentication/command adapter.
pub mod discord;
use serde_json::Value;
use std::{future::Future, pin::Pin};

pub type PlatformFuture<'a, T> = Pin<Box<dyn Future<Output = anyhow::Result<T>> + Send + 'a>>;
pub trait Platform: Send + Sync {
    fn membership<'a>(&'a self, server: &'a str, user: &'a str) -> PlatformFuture<'a, bool>;
    fn notification(
        &self,
        entry: &crate::models::Entry,
        artist: uuid::Uuid,
        recipients: &[String],
        nonce: &str,
    ) -> anyhow::Result<Value>;
    fn send<'a>(&'a self, channel: &'a str, payload: Value) -> PlatformFuture<'a, String>;
}
#[derive(Debug)]
pub struct RetryAfter(pub u64);
impl std::fmt::Display for RetryAfter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Platform rate limit; retry in {} seconds", self.0)
    }
}
impl std::error::Error for RetryAfter {}
