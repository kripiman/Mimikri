// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 kripiman

#![warn(clippy::all)]

mod boot;
pub mod menu;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    boot::cli::binary_health_check().await;
    let args = boot::cli::parse();
    boot::telemetry::init(&args)?;
    boot::runtime::dispatch(args).await
}
