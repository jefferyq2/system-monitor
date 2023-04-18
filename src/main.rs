#![warn(clippy::pedantic)]
pub mod application;
pub mod window;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    window::run().await
}
