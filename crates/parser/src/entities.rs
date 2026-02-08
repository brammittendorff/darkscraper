use once_cell::sync::Lazy;
use regex::Regex;

use darkscraper_core::ExtractedEntities;

static EMAIL_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}").unwrap());

static ONION_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[a-z2-7]{56}\.onion").unwrap());

static I2P_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[a-zA-Z0-9\-]+\.i2p").unwrap());

static BTC_LEGACY_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b[13][a-km-zA-HJ-NP-Z1-9]{25,34}\b").unwrap());

static BTC_BECH32_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\bbc1[a-zA-HJ-NP-Z0-9]{25,89}\b").unwrap());

static MONERO_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b4[0-9AB][1-9A-HJ-NP-Za-km-z]{93}\b").unwrap());

static ETH_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b0x[a-fA-F0-9]{40}\b").unwrap());

static PHONE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\+[1-9]\d{6,14}").unwrap());

// PGP fingerprints: look for specific patterns like "Key fingerprint" or "PGP:" prefix
// to avoid matching random hex strings and blockchain hashes
static PGP_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)(?:key\s+fingerprint|pgp|fingerprint)[:\s=]+([0-9A-Fa-f]{4}[\s]?[0-9A-Fa-f]{4}[\s]?[0-9A-Fa-f]{4}[\s]?[0-9A-Fa-f]{4}[\s]?[0-9A-Fa-f]{4}[\s]?[0-9A-Fa-f]{4}[\s]?[0-9A-Fa-f]{4}[\s]?[0-9A-Fa-f]{4}[\s]?[0-9A-Fa-f]{4}[\s]?[0-9A-Fa-f]{4})",
    )
    .unwrap()
});

// Usernames: require at least 3 chars and exclude common CSS/JS keywords
static USERNAME_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"@[a-zA-Z][a-zA-Z0-9_]{2,49}\b").unwrap());

pub fn extract_entities(text: &str) -> ExtractedEntities {
    let emails = find_unique(&EMAIL_RE, text);
    let onion_addresses = find_unique(&ONION_RE, text);
    let i2p_addresses = find_unique(&I2P_RE, text);

    let mut bitcoin_addresses = find_unique(&BTC_LEGACY_RE, text);
    bitcoin_addresses.extend(find_unique(&BTC_BECH32_RE, text));

    let monero_addresses = find_unique(&MONERO_RE, text);
    let ethereum_addresses = find_unique(&ETH_RE, text);
    let phone_numbers = find_unique(&PHONE_RE, text);
    let pgp_fingerprints = find_unique_with_capture(&PGP_RE, text);
    let usernames = find_unique(&USERNAME_RE, text);

    ExtractedEntities {
        emails,
        onion_addresses,
        i2p_addresses,
        bitcoin_addresses,
        monero_addresses,
        ethereum_addresses,
        phone_numbers,
        pgp_fingerprints,
        usernames,
    }
}

fn find_unique(re: &Regex, text: &str) -> Vec<String> {
    let mut results: Vec<String> = re.find_iter(text).map(|m| m.as_str().to_string()).collect();
    results.sort();
    results.dedup();
    results
}

fn find_unique_with_capture(re: &Regex, text: &str) -> Vec<String> {
    let mut results: Vec<String> = re
        .captures_iter(text)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .collect();
    results.sort();
    results.dedup();
    results
}
