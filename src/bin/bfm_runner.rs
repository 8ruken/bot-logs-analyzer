use anyhow::{bail, Result};

use bot_logs_analyzer::bfm::{
    exp_a_rps, exp_b_concurrency, exp_c_headers, exp_d_url_bias,
    probe::Metrics,
};

fn get_arg(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn parse_csv_f64(s: &str) -> Result<Vec<f64>> {
    Ok(s.split(',').map(|x| x.trim().parse::<f64>()).collect::<std::result::Result<Vec<_>, _>>()?)
}
fn parse_csv_usize(s: &str) -> Result<Vec<usize>> {
    Ok(s.split(',').map(|x| x.trim().parse::<usize>()).collect::<std::result::Result<Vec<_>, _>>()?)
}
fn parse_urls(s: &str) -> Vec<String> {
    s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let exp = get_arg(&args, "--exp").unwrap_or_else(|| "A".to_string());
    let seconds: u64 = get_arg(&args, "--seconds").unwrap_or_else(|| "60".to_string()).parse()?;

    Metrics::print_csv_header();

    match exp.as_str() {
        "A" => {
            let url = get_arg(&args, "--url").ok_or_else(|| anyhow::anyhow!("--url is required for A"))?;
            let concurrency: usize = get_arg(&args, "--concurrency").unwrap_or_else(|| "1".to_string()).parse()?;
            let rps_steps = parse_csv_f64(&get_arg(&args, "--rps-steps").unwrap_or_else(|| "0.2,0.5,1,2,3,5".to_string()))?;
            exp_a_rps::run(url, seconds, rps_steps, concurrency).await?;
        }
        "B" => {
            let url = get_arg(&args, "--url").ok_or_else(|| anyhow::anyhow!("--url is required for B"))?;
            let rps: f64 = get_arg(&args, "--rps").unwrap_or_else(|| "1.0".to_string()).parse()?;
            let conc_steps = parse_csv_usize(&get_arg(&args, "--concurrency-steps").unwrap_or_else(|| "1,2,5,10".to_string()))?;
            exp_b_concurrency::run(url, seconds, rps, conc_steps).await?;
        }
        "C" => {
            let url = get_arg(&args, "--url").ok_or_else(|| anyhow::anyhow!("--url is required for C"))?;
            let rps: f64 = get_arg(&args, "--rps").unwrap_or_else(|| "1.0".to_string()).parse()?;
            let concurrency: usize = get_arg(&args, "--concurrency").unwrap_or_else(|| "1".to_string()).parse()?;
            exp_c_headers::run(url, seconds, rps, concurrency).await?;
        }
        "D" => {
            let urls_s = get_arg(&args, "--urls").ok_or_else(|| anyhow::anyhow!("--urls is required for D"))?;
            let urls = parse_urls(&urls_s);
            if urls.is_empty() {
                bail!("--urls must not be empty");
            }
            let rps: f64 = get_arg(&args, "--rps").unwrap_or_else(|| "1.0".to_string()).parse()?;
            let concurrency: usize = get_arg(&args, "--concurrency").unwrap_or_else(|| "1".to_string()).parse()?;
            exp_d_url_bias::run(urls, seconds, rps, concurrency).await?;
        }
        _ => bail!("--exp must be A|B|C|D"),
    }

    Ok(())
}
