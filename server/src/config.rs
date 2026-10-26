use sha2::{Digest, Sha256};
use std::{collections::HashMap, net::IpAddr, time::Duration};
use subtle::ConstantTimeEq;
use url::Url;

pub struct Config {
    pub(crate) origin: String,
    pub(crate) targets: HashMap<String, Url>,
    pub(crate) route: String,
    pub(crate) limits: Limits,
    token_hash: [u8; 32],
}

#[derive(Clone, Copy)]
pub(crate) struct Limits {
    pub bytes: usize,
    pub inactivity: Duration,
    pub body_timeout: Duration,
    pub concurrency: usize,
    pub rate: usize,
}

impl Config {
    pub fn new(token: &str, origin: &str, endpoints: &[String], base_path: &str) -> Result<Self, &'static str> {
        if !(32..=4096).contains(&token.len())
            || !token.is_ascii()
            || token.bytes().any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
        {
            return Err("A relay token of 32-4096 non-whitespace ASCII bytes is required");
        }
        let public = Url::parse(origin).map_err(|_| "Exact public origin required")?;
        let local = matches!(public.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if public.origin().ascii_serialization() != origin
            || !(public.scheme() == "https" || public.scheme() == "http" && local)
        {
            return Err("Exact HTTPS public origin required (HTTP loopback is allowed for development)");
        }
        if base_path.len() > 512
            || !base_path.starts_with('/')
            || !base_path.ends_with('/')
            || base_path != "/"
                && base_path[1..base_path.len() - 1]
                    .split('/')
                    .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'))
        {
            return Err("Invalid publishing prefix");
        }
        if endpoints.is_empty() || endpoints.len() > 64 {
            return Err("An allowlist of 1-64 complete HTTPS model endpoints is required");
        }
        let mut targets = HashMap::new();
        for value in endpoints {
            let url = endpoint(value)?;
            if targets.insert(url.to_string(), url).is_some() {
                return Err("Duplicate model endpoint");
            }
        }
        Ok(Self {
            origin: origin.to_owned(),
            targets,
            route: format!("{base_path}api/chat/completions"),
            token_hash: Sha256::digest(token.as_bytes()).into(),
            limits: Limits {
                bytes: 48 * 1024 * 1024,
                inactivity: Duration::from_secs(1800),
                body_timeout: Duration::from_secs(180),
                concurrency: 4,
                rate: 60,
            },
        })
    }

    pub(crate) fn authorized(&self, token: &[u8]) -> bool {
        bool::from(self.token_hash.ct_eq(&Sha256::digest(token)[..]))
    }
}

pub(crate) fn endpoint(value: &str) -> Result<Url, &'static str> {
    let url = Url::parse(value).map_err(|_| "Invalid model endpoint")?;
    if value.len() > 4096
        || url.scheme() != "https"
        || url.port_or_known_default() != Some(443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !["/chat/completions", "/v1/messages", "/responses"].iter().any(|suffix| url.path().ends_with(suffix))
    {
        return Err("Model endpoints must be complete HTTPS Chat Completions, Messages or Responses URLs");
    }
    Ok(url)
}

pub(crate) fn is_public(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(ip) => {
            let value = u32::from(ip);
            ![
                ([0, 0, 0, 0], 8),
                ([10, 0, 0, 0], 8),
                ([100, 64, 0, 0], 10),
                ([127, 0, 0, 0], 8),
                ([169, 254, 0, 0], 16),
                ([172, 16, 0, 0], 12),
                ([192, 0, 0, 0], 24),
                ([192, 0, 2, 0], 24),
                ([192, 88, 99, 0], 24),
                ([192, 168, 0, 0], 16),
                ([198, 18, 0, 0], 15),
                ([198, 51, 100, 0], 24),
                ([203, 0, 113, 0], 24),
                ([224, 0, 0, 0], 3),
            ]
            .iter()
            .any(|(base, bits)| value & (u32::MAX << (32 - bits)) == u32::from_be_bytes(*base))
        }
        IpAddr::V6(ip) => {
            let value = u128::from(ip);
            value >> 125 == 1
                && ![
                    (0x2001u128 << 112, 23),
                    (0x20010db8u128 << 96, 32),
                    (0x2002u128 << 112, 16),
                    (0x3fffu128 << 112, 20),
                ]
                .iter()
                .any(|(base, bits)| value & (u128::MAX << (128 - bits)) == *base)
        }
    }
}
