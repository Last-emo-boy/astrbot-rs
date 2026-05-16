use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct TestResponse {
    status: String,
    content_type: String,
    body: Vec<u8>,
}

impl TestResponse {
    pub fn json(status: &str, body: &str) -> Self {
        Self::bytes(status, "application/json", body.as_bytes().to_vec())
    }

    pub fn bytes(status: &str, content_type: &str, body: Vec<u8>) -> Self {
        Self {
            status: status.to_string(),
            content_type: content_type.to_string(),
            body,
        }
    }
}

pub async fn serve_once<B>(
    status: &str,
    content_type: &str,
    body: B,
    captured: Arc<Mutex<String>>,
) -> String
where
    B: Into<Vec<u8>>,
{
    serve_once_response(
        TestResponse::bytes(status, content_type, body.into()),
        captured,
    )
    .await
}

pub async fn serve_sequence(
    responses: Vec<TestResponse>,
    captured: Arc<Mutex<Vec<String>>>,
) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test server should bind");
    let addr = listener.local_addr().expect("test server should have addr");

    tokio::spawn(async move {
        for response in responses {
            let (mut stream, _) = listener.accept().await.expect("server should accept");
            let request = read_http_request(&mut stream).await;
            captured
                .lock()
                .await
                .push(String::from_utf8_lossy(&request).to_string());
            write_response(&mut stream, response).await;
        }
    });

    format!("http://{addr}")
}

async fn serve_once_response(response: TestResponse, captured: Arc<Mutex<String>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test server should bind");
    let addr = listener.local_addr().expect("test server should have addr");

    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("server should accept");
        let request = read_http_request(&mut stream).await;
        *captured.lock().await = String::from_utf8_lossy(&request).to_string();
        write_response(&mut stream, response).await;
    });

    format!("http://{addr}")
}

async fn write_response(stream: &mut TcpStream, response: TestResponse) {
    let headers = format!(
        "HTTP/1.1 {}\r\ncontent-type: {}\r\ncontent-length: {}\r\n\r\n",
        response.status,
        response.content_type,
        response.body.len(),
    );
    stream
        .write_all(headers.as_bytes())
        .await
        .expect("server should write headers");
    stream
        .write_all(&response.body)
        .await
        .expect("server should write body");
}

async fn read_http_request(stream: &mut TcpStream) -> Vec<u8> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];

    loop {
        let read = stream.read(&mut buffer).await.expect("server should read");
        assert_ne!(read, 0, "client closed before sending request");
        request.extend_from_slice(&buffer[..read]);

        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }

    let content_length = content_length(&request);
    let header_end = request
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("request should contain headers")
        + 4;
    while request.len() < header_end + content_length {
        let read = stream.read(&mut buffer).await.expect("server should read");
        assert_ne!(read, 0, "client closed before sending body");
        request.extend_from_slice(&buffer[..read]);
    }

    request
}

fn content_length(request: &[u8]) -> usize {
    let request = String::from_utf8_lossy(request);
    request
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse().ok()
            } else {
                None
            }
        })
        .unwrap_or(0)
}
