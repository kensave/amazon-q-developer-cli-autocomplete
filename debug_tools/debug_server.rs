use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestLog {
    pub id: String,
    pub timestamp: u64,
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: serde_json::Value,
    pub response_status: Option<u16>,
    pub response_headers: Option<HashMap<String, String>>,
    pub response_body: Option<String>,
    pub duration_ms: Option<u64>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug)]
pub struct DebugServer {
    requests: Arc<Mutex<Vec<RequestLog>>>,
    port: u16,
}

impl DebugServer {
    pub fn new(port: u16) -> Self {
        println!("🔍 [SERVER] Creating new debug server instance");
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
            port,
        }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", self.port)).await?;
        println!("🔍 [SERVER] Debug server listening on http://127.0.0.1:{}", self.port);
        println!("📊 [SERVER] Dashboard: http://127.0.0.1:{}/dashboard", self.port);
        
        let requests = Arc::clone(&self.requests);
        
        loop {
            let (mut stream, addr) = listener.accept().await?;
            println!("🔍 [SERVER] New connection from: {}", addr);
            let requests_clone = Arc::clone(&requests);
            
            tokio::spawn(async move {
                let mut buffer = Vec::new();
                let mut temp_buffer = [0; 8192];
                
                // Read the entire request
                loop {
                    match stream.read(&mut temp_buffer).await {
                        Ok(0) => break, // EOF
                        Ok(n) => buffer.extend_from_slice(&temp_buffer[..n]),
                        Err(e) => {
                            println!("❌ [SERVER] Error reading from stream: {}", e);
                            return;
                        }
                    }
                    
                    // Check if we have a complete HTTP request
                    let request_str = String::from_utf8_lossy(&buffer);
                    if let Some(header_end) = request_str.find("\r\n\r\n") {
                        // Parse Content-Length header to know how much body to expect
                        let headers_part = &request_str[..header_end];
                        let mut content_length = 0;
                        
                        for line in headers_part.lines() {
                            if let Some(colon_pos) = line.find(':') {
                                let key = line[..colon_pos].trim().to_lowercase();
                                let value = line[colon_pos + 1..].trim();
                                if key == "content-length" {
                                    content_length = value.parse::<usize>().unwrap_or(0);
                                }
                            }
                        }
                        
                        let expected_total_size = header_end + 4 + content_length;
                        if buffer.len() >= expected_total_size {
                            break; // We have the complete request
                        }
                    }
                }
                
                if buffer.is_empty() {
                    println!("❌ [SERVER] Empty request received");
                    return;
                }
                
                let request_str = String::from_utf8_lossy(&buffer);
                println!("🔍 [SERVER] Complete request received (size: {} bytes)", buffer.len());
                
                let _start_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
                
                // Parse HTTP request
                let mut lines = request_str.lines();
                let request_line = lines.next().unwrap_or("");
                let parts: Vec<&str> = request_line.split_whitespace().collect();
                
                if parts.len() < 3 {
                    println!("❌ [SERVER] Invalid request line: {}", request_line);
                    return;
                }
                
                let method = parts[0].to_string();
                let path = parts[1].to_string();
                println!("🔍 [SERVER] Processing {} request to {}", method, path);
                
                // Parse headers
                let mut headers = HashMap::new();
                
                for (_i, line) in lines.enumerate() {
                    if line.is_empty() {
                        break;
                    }
                    
                    if let Some(colon_pos) = line.find(':') {
                        let key = line[..colon_pos].trim().to_string();
                        let value = line[colon_pos + 1..].trim().to_string();
                        headers.insert(key.clone(), value.clone());
                        println!("🔍 [SERVER] Header: {} = {}", key, value);
                    }
                }
                
                // Extract body
                let body = if let Some(double_crlf) = request_str.find("\r\n\r\n") {
                    let body = request_str[double_crlf + 4..].to_string();
                    println!("🔍 [SERVER] Request body (length: {}):\n{}", body.len(), 
                             if body.len() > 1000 { format!("{}...", &body[..1000]) } else { body.clone() });
                    body
                } else if let Some(double_lf) = request_str.find("\n\n") {
                    let body = request_str[double_lf + 2..].to_string();
                    println!("🔍 [SERVER] Request body (length: {}):\n{}", body.len(), 
                             if body.len() > 1000 { format!("{}...", &body[..1000]) } else { body.clone() });
                    body
                } else {
                    println!("🔍 [SERVER] No request body found");
                    String::new()
                };
                
                let _request_id = Uuid::new_v4().to_string();
                
                // Handle different endpoints
                let (response_body, content_type) = match path.as_str() {
                    "/dashboard" | "/" => {
                        println!("📊 [SERVER] Serving dashboard");
                        (std::fs::read_to_string("dashboard.html").unwrap_or_else(|_| Self::generate_fallback_dashboard_html(&requests_clone)), "text/html")
                    },
                    "/dashboard.css" => {
                        println!("🎨 [SERVER] Serving CSS");
                        (std::fs::read_to_string("dashboard.css").unwrap_or_else(|_| String::from("/* CSS file not found */")), "text/css")
                    },
                    "/dashboard.js" => {
                        println!("📜 [SERVER] Serving JavaScript");
                        (std::fs::read_to_string("dashboard.js").unwrap_or_else(|_| String::from("console.error('JS file not found');")), "application/javascript")
                    },
                    "/api/requests" => {
                        if method == "POST" {
                            println!("🔍 [SERVER] Processing new request log");
                            println!("📄 [SERVER] Request body (first 500 chars): {}", 
                                     if body.len() > 500 { &body[..500] } else { &body });
                            
                            match serde_json::from_str::<RequestLog>(&body) {
                                Ok(log) => {
                                    println!("✅ [SERVER] Successfully parsed request log: {}", log.id);
                                    requests_clone.lock().unwrap().push(log);
                                }
                                Err(e) => {
                                    println!("❌ [SERVER] Failed to parse request log: {}", e);
                                    println!("📄 [SERVER] Full body: {}", body);
                                    
                                    // Display raw body content
                                    println!("📊 [SERVER] Raw body content: {}", body);
                                }
                            }
                        }
                        let requests_guard = requests_clone.lock().unwrap();
                        println!("📊 [SERVER] Returning {} request logs", requests_guard.len());
                        let json = serde_json::to_string(&*requests_guard).unwrap_or_else(|_| "[]".to_string());
                        (json, "application/json")
                    },
                    "/api/streaming-events" => {
                        if method == "POST" {
                            println!("🔍 [SERVER] Processing streaming event");
                            println!("📄 [SERVER] Streaming event body (first 500 chars): {}", 
                                     if body.len() > 500 { &body[..500] } else { &body });
                            
                            match serde_json::from_str::<serde_json::Value>(&body) {
                                Ok(event) => {
                                    println!("✅ [SERVER] Successfully parsed streaming event");
                                    println!("🎯 [SERVER] Event type: {}", 
                                             event.get("event_type").and_then(|v| v.as_str()).unwrap_or("unknown"));
                                    
                                    // Pretty print the streaming event data
                                    if let Some(data) = event.get("data") {
                                        println!("📊 [SERVER] Streaming event data:\n{}", 
                                                 serde_json::to_string_pretty(data).unwrap_or_else(|_| "Failed to format".to_string()));
                                    }
                                }
                                Err(e) => {
                                    println!("❌ [SERVER] Failed to parse streaming event: {}", e);
                                    println!("📄 [SERVER] Full body: {}", body);
                                }
                            }
                        }
                        ("{\"status\": \"received\"}".to_string(), "application/json")
                    },
                    "/api/clear" => {
                        println!("🧹 [SERVER] Clearing all request logs");
                        requests_clone.lock().unwrap().clear();
                        ("{\"status\": \"cleared\"}".to_string(), "application/json")
                    },
                    _ => {
                        println!("❌ [SERVER] Unknown endpoint: {}", path);
                        ("Not Found".to_string(), "text/plain")
                    }
                };
                
                // Send HTTP response
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\nAccess-Control-Allow-Headers: *\r\n\r\n{}",
                    content_type,
                    response_body.len(),
                    response_body
                );
                
                println!("📤 [SERVER] Sending response with content type: {}", content_type);
                match stream.write_all(response.as_bytes()).await {
                    Ok(_) => println!("✅ [SERVER] Response sent successfully"),
                    Err(e) => println!("❌ [SERVER] Failed to send response: {}", e),
                }
            });
        }
    }
    
    fn generate_fallback_dashboard_html(requests: &Arc<Mutex<Vec<RequestLog>>>) -> String {
        println!("📊 [SERVER] Generating fallback dashboard HTML (external files not found)");
        Self::generate_dashboard_html(requests)
    }
    
    fn generate_dashboard_html(requests: &Arc<Mutex<Vec<RequestLog>>>) -> String {
        println!("📊 [SERVER] Generating dashboard HTML");
        let requests_guard = requests.lock().unwrap();
        let requests_json = serde_json::to_string(&*requests_guard).unwrap_or_else(|_| "[]".to_string());
        println!("📊 [SERVER] Current requests: {}", requests_json);
        
        format!(r#"
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Amazon Q Debug Dashboard</title>
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 0; padding: 20px; background: #f5f5f5; }}
        .header {{ background: #232f3e; color: white; padding: 20px; border-radius: 8px; margin-bottom: 20px; }}
        .stats {{ display: flex; gap: 20px; margin-bottom: 20px; }}
        .stat-card {{ background: white; padding: 15px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); flex: 1; }}
        .request-list {{ background: white; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }}
        .request-item {{ border-bottom: 1px solid #eee; padding: 15px; cursor: pointer; }}
        .request-item:hover {{ background: #f9f9f9; }}
        .request-item:last-child {{ border-bottom: none; }}
        .method {{ display: inline-block; padding: 4px 8px; border-radius: 4px; font-size: 12px; font-weight: bold; }}
        .method.POST {{ background: #28a745; color: white; }}
        .method.GET {{ background: #007bff; color: white; }}
        .timestamp {{ color: #666; font-size: 12px; }}
        .details {{ margin-top: 10px; }}
        .json {{ background: #2d3748; color: #e2e8f0; padding: 10px; border-radius: 4px; overflow-x: auto; font-family: 'Monaco', 'Menlo', monospace; font-size: 12px; margin: 5px 0; }}
        .controls {{ margin-bottom: 20px; }}
        button {{ background: #ff9900; color: white; border: none; padding: 10px 20px; border-radius: 4px; cursor: pointer; margin-right: 10px; }}
        button:hover {{ background: #e88b00; }}
        .auto-refresh {{ margin-left: 10px; }}
        .section {{ margin: 10px 0; padding: 10px; background: #f8f9fa; border-radius: 4px; }}
        .section h4 {{ margin: 0 0 10px 0; }}
        .tab-container {{ display: none; }}
        .tab {{ display: none; }}
        .tab.active {{ display: none; }}
        .raw-view {{ font-family: monospace; white-space: pre-wrap; }}
        .copy-button {{ float: right; padding: 4px 8px; font-size: 12px; }}
    </style>
</head>
<body>
    <div class="header">
        <h1>Amazon Q Debug Dashboard</h1>
        <p>Real-time request monitoring for CodeWhisperer streaming client</p>
    </div>
    
    <div class="controls">
        <button onclick="clearRequests()">Clear All</button>
        <button onclick="refreshData()">Refresh</button>
        <button onclick="toggleAllDetails()">Toggle All Details</button>
        <label class="auto-refresh">
            <input type="checkbox" id="autoRefresh" onchange="toggleAutoRefresh()" checked> Auto-refresh (5s)
        </label>
    </div>
    
    <div class="stats">
        <div class="stat-card">
            <h3>Total Requests</h3>
            <div id="totalRequests">0</div>
        </div>
        <div class="stat-card">
            <h3>Recent Activity</h3>
            <div id="recentActivity">No requests yet</div>
        </div>
        <div class="stat-card">
            <h3>Average Response Time</h3>
            <div id="avgResponseTime">-</div>
        </div>
    </div>
    
    <div class="request-list" id="requestList">
        <div style="padding: 20px; text-align: center; color: #666;">
            No requests captured yet. Make some requests to see them here.
        </div>
    </div>

    <script>
        let requests = {requests_json};
        let autoRefreshInterval;
        let showAllDetails = false;
        
        function updateDashboard() {{
            console.log('Updating dashboard with requests:', requests);
            const totalRequests = requests.length;
            document.getElementById('totalRequests').textContent = totalRequests;
            
            if (totalRequests > 0) {{
                const latest = requests[requests.length - 1];
                const latestTime = new Date(latest.timestamp).toLocaleString();
                document.getElementById('recentActivity').textContent = `Last: ${{latest.method}} ${{latest.path}} at ${{latestTime}}`;
                
                const avgTime = requests.filter(r => r.duration_ms).reduce((sum, r) => sum + r.duration_ms, 0) / requests.filter(r => r.duration_ms).length;
                document.getElementById('avgResponseTime').textContent = avgTime ? `${{Math.round(avgTime)}}ms` : '-';
                
                renderRequests();
            }}
        }}
        
        function renderRequests() {{
            console.log('Rendering requests:', requests);
            const container = document.getElementById('requestList');
            if (requests.length === 0) {{
                container.innerHTML = '<div style="padding: 20px; text-align: center; color: #666;">No requests captured yet.</div>';
                return;
            }}
            
            container.innerHTML = requests.slice().reverse().map((req, index) => `
                <div class="request-item" onclick="toggleDetails(${{requests.length - 1 - index}})">
                    <div style="display: flex; justify-content: space-between; align-items: center;">
                        <div>
                            <span class="method ${{req.method}}">${{req.method}}</span>
                            <strong>${{req.path}}</strong>
                            <span class="timestamp">${{new Date(req.timestamp).toLocaleString()}}</span>
                            ${{req.duration_ms ? `<span style="color: #28a745; margin-left: 10px;">${{req.duration_ms}}ms</span>` : ''}}
                        </div>
                        <button onclick="copyRequestData(${{requests.length - 1 - index}}); event.stopPropagation();" class="copy-button">Copy</button>
                    </div>
                    <div class="details" id="details-${{index}}" style="display: none;" onclick="event.stopPropagation()">
                        <div class="section">
                            <h4>JSON Data</h4>
                            <div class="json raw-view">${{JSON.stringify(req, null, 2)}}</div>
                        </div>
                    </div>
                </div>
            `).join('');
        }}
        
        function formatJson(obj) {{
            try {{
                if (typeof obj === 'string') {{
                    return obj; // Return raw string instead of parsing and formatting
                }}
                return JSON.stringify(obj); // Return compact JSON instead of pretty-printed
            }} catch {{
                return JSON.stringify(obj);
            }}
        }}
        
        function copyRequestData(index) {{
            const request = requests[index];
            const text = JSON.stringify(request, null, 2);
            navigator.clipboard.writeText(text).then(() => {{
                alert('Request data copied to clipboard!');
            }}).catch(err => {{
                console.error('Failed to copy:', err);
                alert('Failed to copy request data');
            }});
        }}
        
        function toggleDetails(index) {{
            const detailsElement = document.getElementById(`details-${{requests.length - 1 - index}}`);
            if (detailsElement.style.display === 'none') {{
                detailsElement.style.display = 'block';
            }} else {{
                detailsElement.style.display = 'none';
            }}
        }}
        
        function toggleAllDetails() {{
            showAllDetails = !showAllDetails;
            document.querySelectorAll('.details').forEach(detail => {{
                detail.style.display = showAllDetails ? 'block' : 'none';
            }});
        }}
        
        async function refreshData() {{
            console.log('Refreshing data...');
            try {{
                const response = await fetch('/api/requests');
                const data = await response.json();
                console.log('Received data:', data);
                requests = data;
                updateDashboard();
            }} catch (error) {{
                console.error('Failed to refresh data:', error);
            }}
        }}
        
        async function clearRequests() {{
            console.log('Clearing requests...');
            try {{
                await fetch('/api/clear', {{ method: 'POST' }});
                requests = [];
                updateDashboard();
                // Force a complete page refresh to ensure everything is cleared
                window.location.reload();
            }} catch (error) {{
                console.error('Failed to clear requests:', error);
            }}
        }}
        
        function toggleAutoRefresh() {{
            const checkbox = document.getElementById('autoRefresh');
            if (checkbox.checked) {{
                console.log('Starting auto-refresh');
                autoRefreshInterval = setInterval(refreshData, 5000);
            }} else {{
                console.log('Stopping auto-refresh');
                clearInterval(autoRefreshInterval);
            }}
        }}
        
        // Initialize
        console.log('Initializing dashboard with requests:', requests);
        updateDashboard();
        toggleAutoRefresh();
    </script>
</body>
</html>
        "#, requests_json = requests_json)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = std::env::args()
        .nth(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    
    println!("🚀 [SERVER] Starting debug server on port {}", port);
    let server = DebugServer::new(port);
    server.start().await
}
