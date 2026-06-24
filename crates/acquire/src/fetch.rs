use crate::AcquireError;

/// Abstrai a rede para testabilidade. Produção usa HttpFetcher; testes injetam bytes.
pub trait Fetcher {
    fn fetch(&self, url: &str) -> Result<Vec<u8>, AcquireError>;
}

/// Fetcher HTTP real via reqwest blocking. Envia User-Agent (Packagist exige).
pub struct HttpFetcher {
    client: reqwest::blocking::Client,
}

impl HttpFetcher {
    pub fn new() -> Result<Self, AcquireError> {
        let client = reqwest::blocking::Client::builder()
            .user_agent("phpm/0.1 (+https://github.com/phpm)")
            .build()
            .map_err(|e| AcquireError::Http(e.to_string()))?;
        Ok(HttpFetcher { client })
    }
}

impl Fetcher for HttpFetcher {
    fn fetch(&self, url: &str) -> Result<Vec<u8>, AcquireError> {
        let resp = self
            .client
            .get(url)
            .send()
            .map_err(|e| AcquireError::Http(e.to_string()))?;
        let resp = resp
            .error_for_status()
            .map_err(|e| AcquireError::Http(e.to_string()))?;
        let bytes = resp.bytes().map_err(|e| AcquireError::Http(e.to_string()))?;
        Ok(bytes.to_vec())
    }
}
