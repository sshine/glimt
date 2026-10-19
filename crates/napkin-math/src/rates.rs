//! Exchange rates, from the European Central Bank.
//!
//! The ECB publishes one reference rate per currency against the euro, once per working
//! day, as a small XML file with no key and no terms to agree to. Everything here is
//! units of the currency per euro, which is how the feed states it, and a cross rate
//! goes through the euro in two steps.
//!
//! Reference rates are not what a bank will give you. They are the right answer for
//! napkin arithmetic and the wrong one for a transfer, which is why a result carries the
//! day its rates were published.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;

const FEED: &str = "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml";

/// The euro is the base, so it is not in the feed and has to be put back.
const BASE: &str = "eur";

/// Names people reach for that are not ISO codes.
///
/// The ECB publishes codes, and CNY is the code for the Chinese currency: renminbi is
/// its name and yuan is its unit, so neither appears in the feed. A calculator that
/// answers "ukendt enhed: rmb" to a reasonable thing to type is not much of one.
const ALIASES: [(&str, &str); 3] = [("rmb", "cny"), ("yuan", "cny"), ("euro", "eur")];

/// The code a word means, which is usually the word.
fn canonical(code: &str) -> String {
    let code = code.to_lowercase();

    ALIASES
        .iter()
        .find(|(name, _)| *name == code)
        .map_or(code, |(_, iso)| (*iso).to_owned())
}
/// Where a rate table came from, which decides how much to trust it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Fetched from the ECB during this run.
    Feed,
    /// Read back from disk, published on whatever day it says.
    Cache,
    /// Compiled in. Correct on the day this was written and drifting ever since.
    Baked,
}

#[derive(Debug, Clone)]
pub struct Rates {
    per_eur: BTreeMap<String, f64>,
    pub published: NaiveDate,
    pub source: Source,
}

impl Rates {
    /// The table compiled into the binary: the ECB's own figures for the day this was
    /// written, so the calculator answers on a first run and with no network.
    pub fn baked() -> Self {
        const TABLE: [(&str, f64); 29] = [
            ("usd", 1.1253),
            ("jpy", 177.82),
            ("czk", 24.323),
            ("dkk", 7.4748),
            ("gbp", 0.84385),
            ("huf", 363.08),
            ("pln", 4.3700),
            ("ron", 5.3310),
            ("sek", 11.1450),
            ("chf", 0.9272),
            ("isk", 136.20),
            ("nok", 10.6240),
            ("try", 55.7328),
            ("aud", 1.6171),
            ("brl", 5.7197),
            ("cad", 1.5935),
            ("cny", 7.5240),
            ("hkd", 8.8316),
            ("idr", 20290.28),
            ("ils", 3.4822),
            ("inr", 106.8640),
            ("krw", 1499.01),
            ("mxn", 20.6871),
            ("myr", 4.5929),
            ("nzd", 1.9868),
            ("php", 70.858),
            ("sgd", 1.4395),
            ("thb", 37.602),
            ("zar", 18.4930),
        ];

        let mut per_eur: BTreeMap<String, f64> = TABLE
            .iter()
            .map(|(code, rate)| ((*code).to_owned(), *rate))
            .collect();
        per_eur.insert(BASE.to_owned(), 1.0);

        Self {
            per_eur,
            published: NaiveDate::from_ymd_opt(2026, 10, 16).unwrap_or_default(),
            source: Source::Baked,
        }
    }

    /// Units of `code` per euro.
    pub fn per_eur(&self, code: &str) -> Option<f64> {
        self.per_eur.get(&canonical(code)).copied()
    }

    /// The ISO code a word means, which is what the evaluator is given: it looks rates
    /// up by code, so an alias has to be resolved before it gets there.
    pub fn code(&self, word: &str) -> String {
        canonical(word)
    }

    pub fn is_currency(&self, code: &str) -> bool {
        self.per_eur.contains_key(&canonical(code))
    }

    /// Every code the table knows, for the alist handed to the evaluator.
    pub fn codes(&self) -> impl Iterator<Item = (&str, f64)> {
        self.per_eur
            .iter()
            .map(|(code, rate)| (code.as_str(), *rate))
    }

    /// Parse the ECB's daily file.
    pub fn parse(xml: &str) -> Option<Self> {
        let document = roxmltree::Document::parse(xml).ok()?;

        let mut per_eur = BTreeMap::new();
        per_eur.insert(BASE.to_owned(), 1.0);
        let mut published = None;

        // The file nests three Cube elements: an outer one holding the lot, a dated one
        // per day, and a leaf per currency. Walking every descendant rather than the
        // exact path keeps this working if a level is added.
        for node in document.descendants().filter(|n| n.has_tag_name("Cube")) {
            if let Some(time) = node.attribute("time") {
                published = NaiveDate::parse_from_str(time, "%Y-%m-%d").ok();
            }

            if let (Some(code), Some(rate)) = (node.attribute("currency"), node.attribute("rate")) {
                if let Ok(rate) = rate.parse::<f64>() {
                    per_eur.insert(code.to_lowercase(), rate);
                }
            }
        }

        // One entry is the euro this put there, so anything less is not a rate table.
        if per_eur.len() < 2 {
            return None;
        }

        Some(Self {
            per_eur,
            published: published?,
            source: Source::Feed,
        })
    }

    /// Fetch from the ECB. Blocking, so the caller runs it off the drawing thread.
    pub fn fetch() -> Option<Self> {
        let body = ureq::get(FEED)
            .call()
            .ok()?
            .body_mut()
            .read_to_string()
            .ok()?;

        Rates::parse(&body)
    }

    /// Yesterday's answer is a fine answer; the day before a holiday weekend is the
    /// same answer the ECB itself would give.
    pub fn read_cache(path: &Path) -> Option<Self> {
        let xml = std::fs::read_to_string(path).ok()?;
        let mut rates = Rates::parse(&xml)?;
        rates.source = Source::Cache;
        Some(rates)
    }

    pub fn write_cache(xml: &str, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, xml)
    }

    /// The cache file this process uses.
    pub fn cache_path() -> PathBuf {
        let base = std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
            .unwrap_or_else(|| PathBuf::from("."));

        base.join("glimt").join("eurofxref-daily.xml")
    }

    /// The best table available without going to the network.
    pub fn offline() -> Self {
        Rates::read_cache(&Rates::cache_path()).unwrap_or_else(Rates::baked)
    }

    /// Go to the ECB, keep what comes back, and fall back to what is already on hand.
    pub fn refresh() -> Self {
        let path = Rates::cache_path();

        let fetched = ureq::get(FEED)
            .call()
            .ok()
            .and_then(|mut response| response.body_mut().read_to_string().ok());

        match fetched {
            Some(xml) => match Rates::parse(&xml) {
                Some(rates) => {
                    if let Err(error) = Rates::write_cache(&xml, &path) {
                        tracing::warn!("could not cache the rates: {error}");
                    }
                    rates
                }
                None => Rates::offline(),
            },
            None => Rates::offline(),
        }
    }
}

impl Default for Rates {
    fn default() -> Self {
        Rates::baked()
    }
}

impl Rates {
    /// Whether some currency code starts with this, which is how a half-typed word is
    /// told apart from a wrong one.
    pub fn has_prefix(&self, partial: &str) -> bool {
        let partial = partial.to_lowercase();
        self.per_eur.keys().any(|code| code.starts_with(&partial))
            || ALIASES.iter().any(|(name, _)| name.starts_with(&partial))
    }
}
