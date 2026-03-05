use anyhow::{bail, Result};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, USER_AGENT};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    sync::{mpsc, Semaphore},
    time::{interval, MissedTickBehavior},
};

#[derive(Clone, Copy, Debug)]
pub enum HeaderProfile {
    Good, // UA + Accept + Accept-Language + Cookie
    Bot,  // 短いUA + 余計なヘッダ無し + Cookieなし
}

#[derive(Clone, Copy, Debug)]
pub enum UrlMode {
    Single,
    Rotate,
}

#[derive(Debug)]
pub enum Outcome {
    Http {
        status: u16,
        challenge_like: bool,
        latency_ms: u128,
        cf_ray_present: bool,
    },
    TransportErr,
    BodyReadErr { status: u16, latency_ms: u128 },
}

fn looks_like_challenge(body: &str) -> bool {
    let s = body.to_lowercase();
    s.contains("cf-chl")
        || s.contains("just a moment")
        || s.contains("attention required")
        || s.contains("cloudflare")
}

fn build_client(profile: HeaderProfile) -> Result<reqwest::Client> {
    let mut headers = HeaderMap::new();
    match profile {
        HeaderProfile::Good => {
            headers.insert(USER_AGENT, HeaderValue::from_static("rust-bfm-probe/0.1"));
            headers.insert(ACCEPT, HeaderValue::from_static("text/html,application/xhtml+xml"));
            headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("ja,en-US;q=0.8,en;q=0.6"));
            Ok(reqwest::Client::builder()
                .default_headers(headers)
                .cookie_store(true)
                .redirect(reqwest::redirect::Policy::limited(10))
                .timeout(Duration::from_secs(20))
                .build()?)
        }
        HeaderProfile::Bot => {
            headers.insert(USER_AGENT, HeaderValue::from_static("bot"));
            Ok(reqwest::Client::builder()
                .default_headers(headers)
                .cookie_store(false)
                .redirect(reqwest::redirect::Policy::limited(10))
                .timeout(Duration::from_secs(20))
                .build()?)
        }
    }
}

async fn probe_once(client: reqwest::Client, url: String) -> Outcome {
    let t0 = Instant::now();
    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(_) => return Outcome::TransportErr,
    };

    let status = resp.status().as_u16();
    let cf_ray_present = resp.headers().get("cf-ray").is_some();

    let body = match resp.text().await {
        Ok(t) => t,
        Err(_) => {
            return Outcome::BodyReadErr {
                status,
                latency_ms: t0.elapsed().as_millis(),
            }
        }
    };

    let challenge_like = looks_like_challenge(&body);

    Outcome::Http {
        status,
        challenge_like,
        latency_ms: t0.elapsed().as_millis(),
        cf_ray_present,
    }
}

#[derive(Default, Debug)]
pub struct Metrics {
    pub sent: u64,
    pub recv: u64,

    pub ok200: u64,
    pub status_403: u64,
    pub status_429: u64,
    pub status_5xx: u64,

    pub challenge_like: u64,
    pub transport_err: u64,
    pub body_read_err: u64,

    pub cf_ray_present: u64,
    pub latencies_ms: Vec<u128>,
}

fn percentile_p50_p95(v: &Vec<u128>) -> (u128, u128) {
    if v.is_empty() {
        return (0, 0);
    }
    let mut a = v.clone();
    a.sort_unstable();
    let p50_i = (a.len() as f64 * 0.50).floor() as usize;
    let p95_i = (a.len() as f64 * 0.95).floor() as usize;
    let p50 = a[p50_i.min(a.len() - 1)];
    let p95 = a[p95_i.min(a.len() - 1)];
    (p50, p95)
}

impl Metrics {
    pub fn print_csv_header() {
        println!("exp,profile,url_mode,rps,concurrency,seconds,sent,recv,ok200,status403,status429,status5xx,challenge_like,transport_err,body_read_err,cf_ray_present,p50_ms,p95_ms");
    }

    pub fn print_csv_row(
        &self,
        exp: &str,
        profile: HeaderProfile,
        url_mode: UrlMode,
        rps: f64,
        concurrency: usize,
        seconds: u64,
    ) {
        let (p50, p95) = percentile_p50_p95(&self.latencies_ms);
        println!(
            "{},{:?},{:?},{:.3},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            exp,
            profile,
            url_mode,
            rps,
            concurrency,
            seconds,
            self.sent,
            self.recv,
            self.ok200,
            self.status_403,
            self.status_429,
            self.status_5xx,
            self.challenge_like,
            self.transport_err,
            self.body_read_err,
            self.cf_ray_present,
            p50,
            p95
        );
    }
}

/// 共通の「1ステップ（rps/concurrency/seconds固定）」を実行して Metrics を返す
pub async fn run_step(
    profile: HeaderProfile,
    urls: Vec<String>,
    url_mode: UrlMode,
    rps: f64,
    concurrency: usize,
    seconds: u64,
) -> Result<Metrics> {
    // 安全上限（過負荷にしない）
    let rps = rps.clamp(0.05, 5.0);
    let concurrency = concurrency.clamp(1, 10);
    let seconds = seconds.clamp(5, 300);

    if urls.is_empty() {
        bail!("urls must not be empty");
    }

    let client = build_client(profile)?;
    let sem = Arc::new(Semaphore::new(concurrency));
    let (tx, mut rx) = mpsc::unbounded_channel::<Outcome>();

    let idx = Arc::new(AtomicUsize::new(0));

    let step_dur = Duration::from_secs(seconds);
    let start = Instant::now();

    let mut tick = interval(Duration::from_secs_f64(1.0 / rps));
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);

    let mut metrics = Metrics::default();

    while start.elapsed() < step_dur {
        tick.tick().await;
        metrics.sent += 1;

        let permit = sem.clone().acquire_owned().await?;
        let client_cloned = client.clone();
        let tx_cloned = tx.clone();

        let url = match url_mode {
            UrlMode::Single => urls[0].clone(),
            UrlMode::Rotate => {
                let i = idx.fetch_add(1, Ordering::Relaxed) % urls.len();
                urls[i].clone()
            }
        };

        tokio::spawn(async move {
            let _permit = permit;
            let out = probe_once(client_cloned, url).await;
            let _ = tx_cloned.send(out);
        });
    }

    drop(tx);

    while let Some(out) = rx.recv().await {
        metrics.recv += 1;
        match out {
            Outcome::Http {
                status,
                challenge_like,
                latency_ms,
                cf_ray_present,
            } => {
                metrics.latencies_ms.push(latency_ms);
                if cf_ray_present {
                    metrics.cf_ray_present += 1;
                }
                match status {
                    200 => metrics.ok200 += 1,
                    403 => metrics.status_403 += 1,
                    429 => metrics.status_429 += 1,
                    s if (500..=599).contains(&s) => metrics.status_5xx += 1,
                    _ => {}
                }
                if challenge_like {
                    metrics.challenge_like += 1;
                }
            }
            Outcome::TransportErr => metrics.transport_err += 1,
            Outcome::BodyReadErr { status, latency_ms } => {
                metrics.body_read_err += 1;
                metrics.latencies_ms.push(latency_ms);
                match status {
                    200 => metrics.ok200 += 1,
                    403 => metrics.status_403 += 1,
                    429 => metrics.status_429 += 1,
                    s if (500..=599).contains(&s) => metrics.status_5xx += 1,
                    _ => {}
                }
            }
        }
    }

    Ok(metrics)
}
