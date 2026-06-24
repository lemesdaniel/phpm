use acquire::{AcquireError, Fetcher};

/// Fetcher de teste que devolve bytes fixos, sem rede.
struct StaticFetcher {
    bytes: Vec<u8>,
}

impl Fetcher for StaticFetcher {
    fn fetch(&self, _url: &str) -> Result<Vec<u8>, AcquireError> {
        Ok(self.bytes.clone())
    }
}

#[test]
fn static_fetcher_returns_bytes() {
    let f = StaticFetcher { bytes: vec![1, 2, 3] };
    assert_eq!(f.fetch("http://x").unwrap(), vec![1, 2, 3]);
}
