use anyhow::Result;
use super::probe::{run_step, HeaderProfile, Metrics, UrlMode};

pub async fn run(urls: Vec<String>, seconds: u64, rps: f64, concurrency: usize) -> Result<()> {
    let profile = HeaderProfile::Bot;

    // Single: urls[0] を連打 / Rotate: urls をローテーション
    for mode in [UrlMode::Single, UrlMode::Rotate] {
        let m: Metrics = run_step(profile, urls.clone(), mode, rps, concurrency, seconds).await?;
        m.print_csv_row("D", profile, mode, rps, concurrency, seconds);
    }
    Ok(())
}
