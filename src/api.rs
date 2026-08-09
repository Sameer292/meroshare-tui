use anyhow::{anyhow, Context};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde_json::{json, Value};
use std::env;

use crate::db::Account;

fn headers() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert("Content-Type", HeaderValue::from_static("application/json"));
    h.insert(
        "Accept",
        HeaderValue::from_static("application/json, text/plain, */*"),
    );
    h.insert(
        "Referer",
        HeaderValue::from_static("https://meroshare.cdsc.com.np/"),
    );
    h.insert(
        "User-Agent",
        HeaderValue::from_static("Mozilla/5.0 (X11; Linux x86_64) Chrome/144.0.0.0 Safari/537.36"),
    );
    h
}

fn f64_at(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(|x| x.as_f64()).unwrap_or(0.0)
}

fn str_at(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(String::from)
        .unwrap_or_default()
}

#[derive(Debug, Clone)]
pub struct Holding {
    pub script: String,
    pub company: String,
    pub qty: f64,
    pub prev_close: f64,
    pub val_prev_close: f64,
    pub ltp: f64,
    pub val_ltp: f64,
}

#[derive(Debug, Clone)]
pub struct PortfolioSummary {
    pub name: String,
    pub username: String,
    pub email: String,
    pub total_shares: f64,
    pub prev_close: f64,
    pub last_traded: f64,
    pub profit_loss: f64,
    pub holdings: Vec<Holding>,
}

pub fn login(client_id: i64, username: &str, password: &str, base: &str) -> anyhow::Result<String> {
    let client = Client::builder().default_headers(headers()).build()?;
    let res = client
        .post(format!("{base}/meroShare/auth/"))
        .json(&json!({
            "clientId": client_id,
            "username": username,
            "password": password,
        }))
        .send()
        .context("login request failed")?;
    if !res.status().is_success() {
        return Err(anyhow!("login failed: HTTP {}", res.status()));
    }
    let token = res
        .headers()
        .get(AUTHORIZATION)
        .context("no authorization header in login response")?
        .to_str()
        .context("invalid authorization header")?
        .to_string();
    Ok(token)
}

pub fn get_own_detail(token: &str, base: &str) -> anyhow::Result<Value> {
    let client = Client::builder().default_headers(headers()).build()?;
    let res = client
        .get(format!("{base}/meroShare/ownDetail/"))
        .header(AUTHORIZATION, token)
        .send()
        .context("own detail request failed")?;
    if !res.status().is_success() {
        return Err(anyhow!("own detail failed: HTTP {}", res.status()));
    }
    res.json().context("invalid own detail JSON")
}

pub fn get_portfolio(
    token: &str,
    demat: &[String],
    client_code: &str,
    base: &str,
) -> anyhow::Result<Value> {
    let client = Client::builder().default_headers(headers()).build()?;
    let res = client
        .post(format!("{base}/meroShareView/myPortfolio/"))
        .header(AUTHORIZATION, token)
        .json(&json!({
            "sortBy": "script",
            "demat": demat,
            "clientCode": client_code,
            "page": 1,
            "size": 200,
            "sortAsc": true,
        }))
        .send()
        .context("portfolio request failed")?;
    if !res.status().is_success() {
        return Err(anyhow!("portfolio failed: HTTP {}", res.status()));
    }
    res.json().context("invalid portfolio JSON")
}

fn parse_portfolio(data: Value, own_detail: Value) -> PortfolioSummary {
    let holdings: Vec<Holding> = data
        .get("meroShareMyPortfolio")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .map(|h| Holding {
                    script: str_at(h, "script"),
                    company: str_at(h, "scriptDesc"),
                    qty: f64_at(h, "currentBalance"),
                    prev_close: str_at(h, "previousClosingPrice").parse().unwrap(),
                    val_prev_close: f64_at(h, "valueOfPrevClosingPrice"),
                    ltp: str_at(h, "lastTransactionPrice").parse().unwrap(),
                    val_ltp: f64_at(h, "valueOfLastTransPrice"),
                })
                .collect()
        })
        .unwrap_or_default();
    let mut holdings = holdings;
    holdings.sort_by_key(|h| h.script.to_lowercase());

    let total_shares = holdings.iter().map(|h| h.qty).sum();
    let prev_close = f64_at(&data, "totalValueOfPrevClosingPrice");
    let last_traded = f64_at(&data, "totalValueOfLastTransPrice");

    PortfolioSummary {
        name: String::new(),
        username: String::new(),
        email: str_at(&own_detail, "email"),
        total_shares,
        prev_close,
        last_traded,
        profit_loss: last_traded - prev_close,
        holdings,
    }
}

pub fn fetch_account(acc: &Account) -> anyhow::Result<PortfolioSummary> {
    let base: String = env::var("BASE_URL").expect("Base Url must be set");
    let token = login(acc.client_id, &acc.username, &acc.password, &base)?;
    let data = get_portfolio(
        &token,
        std::slice::from_ref(&acc.demat),
        &acc.client_code,
        &base,
    )?;
    let own_detail = get_own_detail(&token, &base)?;
    let mut summary = parse_portfolio(data, own_detail);
    summary.name = acc.name.clone();
    summary.username = acc.username.clone();
    Ok(summary)
}
