use base64::Engine;
use reqwest::blocking::Client;
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Clone)]
pub struct CompressResult {
    pub file_name: String,
    pub original_size: u64,
    pub compressed_size: u64,
    pub saved_to: String,
    pub compression_count: u64,
}

pub struct TinyPngClient {
    #[cfg(not(debug_assertions))]
    client: Client,
    api_key: String,
}

impl TinyPngClient {
    pub fn new(api_key: &str) -> Self {
        Self {
            #[cfg(not(debug_assertions))]
            client: Client::builder()
                .use_rustls_tls()
                .build()
                .expect("Failed to build HTTP client"),
            api_key: api_key.to_string(),
        }
    }

    // ── Mock implementations (dev builds) ────────────────────

    #[cfg(debug_assertions)]
    pub fn validate_key(&self) -> Result<(bool, u64), String> {
        let preview = &self.api_key[..4.min(self.api_key.len())];
        eprintln!("[MOCK] validate_key for '{}...'", preview);
        Ok((true, 42))
    }

    #[cfg(debug_assertions)]
    pub fn compress_file(
        &self,
        file_path: &Path,
        output_dir: &Path,
    ) -> Result<CompressResult, String> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static MOCK_COUNT: AtomicU64 = AtomicU64::new(42);

        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let file_data =
            fs::read(file_path).map_err(|e| format!("Failed to read {}: {}", file_name, e))?;
        let original_size = file_data.len() as u64;

        eprintln!("[MOCK] compressing {} ({} bytes)", file_name, original_size);

        // Simulate network delay
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Save original file to output dir (mock — no actual compression)
        fs::create_dir_all(output_dir)
            .map_err(|e| format!("Failed to create output dir: {}", e))?;
        let output_path = output_dir.join(&file_name);
        fs::write(&output_path, &file_data)
            .map_err(|e| format!("Failed to save {}: {}", file_name, e))?;

        // Fake a 20-40% size reduction for realistic UI testing
        let fake_ratio = 0.6 + (original_size % 20) as f64 * 0.01;
        let compressed_size = (original_size as f64 * fake_ratio) as u64;
        let count = MOCK_COUNT.fetch_add(1, Ordering::Relaxed);

        Ok(CompressResult {
            file_name,
            original_size,
            compressed_size,
            saved_to: output_path.to_string_lossy().to_string(),
            compression_count: count,
        })
    }

    // ── Real implementations (release builds) ────────────────

    #[cfg(not(debug_assertions))]
    fn auth_header(&self) -> String {
        let credentials = format!("api:{}", self.api_key);
        let encoded = base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes());
        format!("Basic {}", encoded)
    }

    #[cfg(not(debug_assertions))]
    pub fn validate_key(&self) -> Result<(bool, u64), String> {
        let resp = self
            .client
            .post("https://api.tinify.com/shrink")
            .header("Authorization", self.auth_header())
            .header("Content-Type", "application/octet-stream")
            .body(Vec::<u8>::new())
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = resp.status().as_u16();
        let count = resp
            .headers()
            .get("Compression-Count")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);

        match status {
            201 => Ok((true, count)),
            400 => Ok((true, count)),
            401 => Err("Invalid API key".to_string()),
            429 => Err("Rate limit exceeded — try again later".to_string()),
            _ => {
                let body = resp.text().unwrap_or_default();
                Err(format!("Unexpected response ({}): {}", status, body))
            }
        }
    }

    #[cfg(not(debug_assertions))]
    pub fn compress_file(
        &self,
        file_path: &Path,
        output_dir: &Path,
    ) -> Result<CompressResult, String> {
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let file_data =
            fs::read(file_path).map_err(|e| format!("Failed to read {}: {}", file_name, e))?;
        let original_size = file_data.len() as u64;

        let upload_resp = self
            .client
            .post("https://api.tinify.com/shrink")
            .header("Authorization", self.auth_header())
            .body(file_data)
            .send()
            .map_err(|e| format!("Upload failed for {}: {}", file_name, e))?;

        let status = upload_resp.status().as_u16();
        let compression_count = upload_resp
            .headers()
            .get("Compression-Count")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);

        let location = upload_resp
            .headers()
            .get("Location")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_string());

        if status == 401 {
            return Err("Invalid API key".to_string());
        }

        if status != 201 {
            let body = upload_resp.text().unwrap_or_default();
            return Err(format!("Compression failed ({}): {}", status, body));
        }

        let download_url =
            location.ok_or_else(|| "No Location header in response".to_string())?;

        let download_resp = self
            .client
            .get(&download_url)
            .header("Authorization", self.auth_header())
            .send()
            .map_err(|e| format!("Download failed for {}: {}", file_name, e))?;

        if !download_resp.status().is_success() {
            return Err(format!(
                "Download failed for {} with status {}",
                file_name,
                download_resp.status()
            ));
        }

        let compressed_data = download_resp
            .bytes()
            .map_err(|e| format!("Failed to read response for {}: {}", file_name, e))?;
        let compressed_size = compressed_data.len() as u64;

        fs::create_dir_all(output_dir)
            .map_err(|e| format!("Failed to create output dir: {}", e))?;

        let output_path = output_dir.join(&file_name);
        fs::write(&output_path, &compressed_data)
            .map_err(|e| format!("Failed to save {}: {}", file_name, e))?;

        Ok(CompressResult {
            file_name,
            original_size,
            compressed_size,
            saved_to: output_path.to_string_lossy().to_string(),
            compression_count,
        })
    }
}
