//! Exchange rates for currency conversion, from a free, keyless endpoint. Only the base
//! currency is ever sent. Cached in rates.json for 12 hours.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::store::{now_ms, read_json, write_json};

const URL: &str = "https://open.er-api.com/v6/latest/USD";
const FRESH_MS: i64 = 12 * 3600 * 1000;
/// After a failed download, wait this long before trying again (offline machines).
const RETRY_AFTER: Duration = Duration::from_secs(120);

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RatesData {
    pub base: String,
    pub rates: HashMap<String, f64>,
    /// When they were downloaded (ms since the epoch).
    pub fetched: i64,
}

#[derive(Default)]
struct State {
    data: Option<RatesData>,
    loaded_file: bool,
    last_attempt: Option<Instant>,
    offline: bool,
}

#[derive(Default)]
pub struct Rates {
    state: Mutex<State>,
}

impl Rates {
    /// Never downloads (tests).
    pub fn offline() -> Self {
        Rates { state: Mutex::new(State { loaded_file: true, offline: true, ..Default::default() }) }
    }

    pub fn with_rates(rates: &[(&str, f64)]) -> Self {
        let data = RatesData { base: "USD".into(), rates: rates.iter().map(|(k, v)| (k.to_string(), *v)).collect(), fetched: now_ms() };
        Rates { state: Mutex::new(State { data: Some(data), loaded_file: true, offline: true, ..Default::default() }) }
    }

    /// Current rates, downloading them if the cache is stale. Blocks (up to a few seconds) while
    /// downloading, so call it off the UI thread; concurrent callers wait for the same download.
    pub fn get(&self) -> Option<RatesData> {
        let mut st = self.state.lock().unwrap();
        let fresh = |d: &Option<RatesData>| d.as_ref().is_some_and(|d| now_ms() - d.fetched < FRESH_MS);
        if fresh(&st.data) {
            return st.data.clone();
        }
        if !st.loaded_file {
            st.loaded_file = true;
            if let Some(d) = read_json::<RatesData>("rates.json") {
                st.data = Some(d);
                if fresh(&st.data) {
                    return st.data.clone();
                }
            }
        }
        if st.offline || st.last_attempt.is_some_and(|t| t.elapsed() < RETRY_AFTER) {
            return st.data.clone();
        }
        st.last_attempt = Some(Instant::now());
        if let Some(d) = download() {
            let _ = write_json("rates.json", &d, None);
            st.data = Some(d);
        }
        st.data.clone()
    }
}

fn download() -> Option<RatesData> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_millis(3500))).build().into();
    let body = agent.get(URL).call().ok()?.body_mut().read_to_string().ok()?;
    #[derive(Deserialize)]
    struct Reply {
        result: Option<String>,
        rates: Option<HashMap<String, f64>>,
    }
    let reply: Reply = serde_json::from_str(&body).ok()?;
    if reply.result.as_deref() != Some("success") {
        return None;
    }
    Some(RatesData { base: "USD".into(), rates: reply.rates?, fetched: now_ms() })
}
