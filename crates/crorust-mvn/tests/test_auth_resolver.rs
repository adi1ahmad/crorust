use anyhow::Result;
use crorust_mvn::{
    Download, MavenCoordinates, RepositoryClient, RepositoryHttpCredentials, SemverVersion,
};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

struct TestHttpClient;

impl Download for TestHttpClient {
    fn download(&self, url: &str, dest: &Path) -> Result<()> {
        self.download_with_auth(url, dest, None)
    }

    fn download_with_auth(
        &self,
        url: &str,
        dest: &Path,
        auth: Option<&RepositoryHttpCredentials>,
    ) -> Result<()> {
        let parsed = url
            .strip_prefix("http://")
            .ok_or_else(|| anyhow::anyhow!("Only http supported in test"))?;
        let slash_pos = parsed.find('/').unwrap_or(parsed.len());
        let host_port = &parsed[..slash_pos];
        let path = if slash_pos < parsed.len() {
            &parsed[slash_pos..]
        } else {
            "/"
        };

        let mut stream = std::net::TcpStream::connect(host_port)?;
        let mut req = format!("GET {path} HTTP/1.1\r\nHost: {host_port}\r\nConnection: close\r\n");

        if let Some(auth) = auth {
            match auth {
                RepositoryHttpCredentials::Basic { username, password } => {
                    let creds = format!("{username}:{password}");
                    let b64 = base64::encode(&creds);
                    req.push_str(&format!("Authorization: Basic {b64}\r\n"));
                }
                RepositoryHttpCredentials::Bearer { token } => {
                    req.push_str(&format!("Authorization: Bearer {token}\r\n"));
                }
            }
        }
        req.push_str("\r\n");

        stream.write_all(req.as_bytes())?;

        let mut resp = Vec::new();
        stream.read_to_end(&mut resp)?;

        let resp_str = String::from_utf8_lossy(&resp);
        let first_line = resp_str.lines().next().unwrap_or("");
        if !first_line.contains("200 OK") {
            anyhow::bail!("HTTP status error: {first_line}");
        }

        let header_end = resp
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .map(|pos| pos + 4)
            .ok_or_else(|| anyhow::anyhow!("Malformed HTTP response"))?;

        let body = &resp[header_end..];
        std::fs::write(dest, body)?;
        Ok(())
    }
}

fn spawn_mock_auth_server() -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Bind ephemeral port");
    let addr = listener.local_addr().expect("Local address");
    let request_count = Arc::new(AtomicUsize::new(0));
    let req_counter = request_count.clone();

    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = match stream {
                Ok(s) => s,
                Err(_) => break,
            };
            req_counter.fetch_add(1, Ordering::SeqCst);

            let mut buf = [0u8; 4096];
            let n = match stream.read(&mut buf) {
                Ok(n) if n > 0 => n,
                _ => continue,
            };
            let req_str = String::from_utf8_lossy(&buf[..n]);

            let is_authorized =
                req_str.contains("Authorization: Basic dGVzdHVzZXI6c2VjcmV0cGFzcw==");

            if !is_authorized {
                let resp = "HTTP/1.1 401 Unauthorized\r\nContent-Length: 12\r\nConnection: close\r\n\r\nUnauthorized";
                let _ = stream.write_all(resp.as_bytes());
                continue;
            }

            if req_str.contains("/com/enterprise/corp-sdk/maven-metadata.xml") {
                let xml = r#"<metadata><groupId>com.enterprise</groupId><artifactId>corp-sdk</artifactId><versioning><latest>2.0.0</latest><release>2.0.0</release><versions><version>2.0.0</version></versions></versioning></metadata>"#;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    xml.len(),
                    xml
                );
                let _ = stream.write_all(resp.as_bytes());
            } else if req_str.contains("/com/enterprise/corp-sdk/2.0.0/corp-sdk-2.0.0.pom") {
                let pom = r#"<project><groupId>com.enterprise</groupId><artifactId>corp-sdk</artifactId><version>2.0.0</version><packaging>jar</packaging><dependencies></dependencies></project>"#;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    pom.len(),
                    pom
                );
                let _ = stream.write_all(resp.as_bytes());
            } else if req_str.contains("/com/enterprise/corp-sdk/2.0.0/corp-sdk-2.0.0.jar") {
                let jar_bytes = b"PK\x03\x04mock_corp_sdk_bytes";
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/java-archive\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    jar_bytes.len()
                );
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.write_all(jar_bytes);
            } else {
                let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\nConnection: close\r\n\r\nNot Found";
                let _ = stream.write_all(resp.as_bytes());
            }
        }
    });

    (format!("http://127.0.0.1:{}", addr.port()), request_count)
}

#[test]
fn test_maven_auth_resolution_success() -> Result<()> {
    let (server_url, _) = spawn_mock_auth_server();
    let temp_dir = tempfile::tempdir()?;
    let cache = temp_dir.path();

    let mut client = RepositoryClient::new(cache.to_path_buf(), TestHttpClient)?;
    client.add_repository_with_auth(
        &server_url,
        RepositoryHttpCredentials::Basic {
            username: "testuser".to_string(),
            password: "secretpass".to_string(),
        },
    );

    let pkg = MavenCoordinates::new("com.enterprise", "corp-sdk");
    let ver = SemverVersion::from_str("2.0.0")?;
    let paths = client.resolve(pkg, ver)?;

    assert_eq!(paths.len(), 1);
    let resolved_path = &paths[0];
    assert!(resolved_path.exists());
    let contents = std::fs::read(resolved_path)?;
    assert_eq!(contents, b"PK\x03\x04mock_corp_sdk_bytes");

    Ok(())
}

#[test]
fn test_maven_unauthenticated_request_rejected() -> Result<()> {
    let (server_url, _) = spawn_mock_auth_server();
    let temp_dir = tempfile::tempdir()?;
    let cache = temp_dir.path();

    let mut client = RepositoryClient::new(cache.to_path_buf(), TestHttpClient)?;

    client.add_repository(&server_url);

    let pkg = MavenCoordinates::new("com.enterprise", "corp-sdk");
    let ver = SemverVersion::from_str("2.0.0")?;
    let result = client.resolve(pkg, ver);

    assert!(
        result.is_err(),
        "Resolution should fail when 401 Unauthorized is returned"
    );
    Ok(())
}
