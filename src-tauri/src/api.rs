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
    client: Client,
    api_key: String,
}

impl TinyPngClient {
    pub fn new(api_key: &str) -> Self {
        Self {
            client: Client::builder()
                .use_rustls_tls()
                .build()
                .expect("Failed to build HTTP client"),
            api_key: api_key.to_string(),
        }
    }

    fn auth_header(&self) -> String {
        let credentials = format!("api:{}", self.api_key);
        let encoded = base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes());
        format!("Basic {}", encoded)
    }

    /// Validate the API key by making a request to the API.
    /// Returns (is_valid, compression_count).
    pub fn validate_key(&self) -> Result<(bool, u64), String> {
        // Send a minimal POST to check auth. We send an empty body on purpose:
        // - 401 = invalid key
        // - 400 = key is valid, but image data is bad (expected)
        // - 201 = key is valid and image was compressed
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
            // 400 means auth succeeded but the (empty) image was rejected — key is valid
            400 => Ok((true, count)),
            401 => Err("Invalid API key".to_string()),
            429 => Err("Rate limit exceeded — try again later".to_string()),
            _ => {
                let body = resp.text().unwrap_or_default();
                Err(format!("Unexpected response ({}): {}", status, body))
            }
        }
    }

    /// Compress a file and save the result.
    /// Returns CompressResult on success.
    pub fn compress_file(&self, file_path: &Path, output_dir: &Path) -> Result<CompressResult, String> {
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Read the source file
        let file_data =
            fs::read(file_path).map_err(|e| format!("Failed to read {}: {}", file_name, e))?;
        let original_size = file_data.len() as u64;

        // Step 1: Upload to TinyPNG
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

        // Step 2: Download compressed file
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

        // Step 3: Save to output directory
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
