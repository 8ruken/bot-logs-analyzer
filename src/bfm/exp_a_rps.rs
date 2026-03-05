use anyhow::Result;
use super::probe::{run_step, HeaderProfile, Metrics, UrlMode};

pub async fn run(url: String, seconds: u64, rps_steps: Vec<f64>, concurrency: usize) -> Result<()> {
    let profile = HeaderProfile::Bot;
    let urls = vec![url];

    for rps in rps_steps {
        let m: Metrics = run_step(profile, urls.clone(), UrlMode::Single, rps, concurrency, seconds).await?;
        m.print_csv_row("A", profile, UrlMode::Single, rps, concurrency, seconds);
    }
    Ok(())
}
