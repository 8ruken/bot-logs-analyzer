use anyhow::Result;
use super::probe::{run_step, HeaderProfile, Metrics, UrlMode};

pub async fn run(url: String, seconds: u64, rps: f64, conc_steps: Vec<usize>) -> Result<()> {
    let profile = HeaderProfile::Bot;
    let urls = vec![url];

    for c in conc_steps {
        let m: Metrics = run_step(profile, urls.clone(), UrlMode::Single, rps, c, seconds).await?;
        m.print_csv_row("B", profile, UrlMode::Single, rps, c, seconds);
    }
    Ok(())
}
