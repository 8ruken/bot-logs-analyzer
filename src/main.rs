use anyhow::Result;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use scraper::{Html, Selector};

#[tokio::main]
async fn main() -> Result<()> {
    let url = "https://blog.haruken.dev/ja/";

    // ちゃんと User-Agent を入れる（弾かれにくくなる）
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("rust-scraper/0.1"));

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;

    let html = client.get(url).send().await?.error_for_status()?.text().await?;

    let document = Html::parse_document(&html);
    let sel = Selector::parse("title").unwrap();

    let title = document
        .select(&sel)
        .next()
        .map(|n| n.text().collect::<String>())
        .unwrap_or_else(|| "(no title)".to_string());

    println!("title = {}", title);
    Ok(())
}
