use anyhow::Result;
use super::probe::{run_step, HeaderProfile, Metrics, UrlMode};

pub async fn run(url: String, seconds: u64, rps: f64, concurrency: usize) -> Result<()> {
    let urls = vec![url];

    for profile in [HeaderProfile::Good, HeaderProfile::Bot] {
        let m: Metrics = run_step(profile, urls.clone(), UrlMode::Single, rps, concurrency, seconds).await?;
        m.print_csv_row("C", profile, UrlMode::Single, rps, concurrency, seconds);
    }
    Ok(())
}
