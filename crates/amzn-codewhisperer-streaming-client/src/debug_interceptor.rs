//! Debug interceptor for capturing and logging HTTP request data
//! 
//! This module provides debugging capabilities for the CodeWhisperer streaming client
//! by intercepting HTTP requests and sending unredacted data to a debug server.

use aws_smithy_runtime_api::client::interceptors::Intercept;
use aws_smithy_runtime_api::client::interceptors::context::{
    BeforeSerializationInterceptorContextRef,
    FinalizerInterceptorContextRef,
    BeforeTransmitInterceptorContextRef,
};
use aws_smithy_runtime_api::client::runtime_components::RuntimeComponents;
use aws_smithy_types::config_bag::ConfigBag;

/// Debug interceptor plugin for capturing and logging request data
#[derive(Debug, Clone)]
pub struct DebugInterceptorPlugin;

impl DebugInterceptorPlugin {
    /// Create a new debug interceptor plugin
    pub fn new() -> Self {
        Self
    }
}

/// Debug interceptor that captures request data
#[derive(Debug)]
pub struct DebugInterceptor;

impl Intercept for DebugInterceptor {
    fn name(&self) -> &'static str {
        "DebugInterceptor"
    }

    fn read_before_execution(
        &self,
        _context: &BeforeSerializationInterceptorContextRef<'_>,
        _config: &mut ConfigBag,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        #[cfg(feature = "debug-requests")]
        {
            // The main debug data will be sent from read_before_transmit
            // This method is kept for potential fallback or additional debugging
        }

        Ok(())
    }

    fn read_before_serialization(
        &self,
        _context: &BeforeSerializationInterceptorContextRef<'_>,
        _runtime_components: &RuntimeComponents,
        _cfg: &mut ConfigBag,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // We'll get the unredacted data in read_before_transmit instead
        // This method is kept for potential future use
        Ok(())
    }

    fn read_before_transmit(
        &self,
        context: &BeforeTransmitInterceptorContextRef<'_>,
        _runtime_components: &RuntimeComponents,
        _cfg: &mut ConfigBag,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        #[cfg(feature = "debug-requests")]
        {
            // Extract the unredacted HTTP request body and send to debug server
            let request = context.request();
            
            if let Some(body) = request.body().bytes() {
                if let Ok(body_str) = std::str::from_utf8(body) {
                    // Try to parse as JSON to get structured data
                    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(body_str) {
                        self.send_unredacted_data_to_debug_server(json_value, request);
                    }
                }
            }
        }
        
        Ok(())
    }

    fn read_after_execution(
        &self,
        _context: &FinalizerInterceptorContextRef<'_>,
        _components: &RuntimeComponents,
        _config: &mut ConfigBag,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Execution completed - could add response logging here if needed
        Ok(())
    }
}

impl DebugInterceptor {
    /// Send the unredacted request data to the debug server
    #[cfg(feature = "debug-requests")]
    fn send_unredacted_data_to_debug_server(&self, json_body: serde_json::Value, request: &aws_smithy_runtime_api::http::Request) {
        use std::collections::HashMap;
        use uuid::Uuid;

        let request_id = Uuid::new_v4().to_string();
        let start_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Extract headers
        let mut headers = HashMap::new();
        for (name, value) in request.headers() {
            headers.insert(name.to_string(), value.to_string());
        }

        let debug_payload = serde_json::json!({
            "id": request_id,
            "timestamp": start_time,
            "method": request.method().to_string(),
            "path": request.uri().to_string(),
            "headers": headers,
            "body": {
                "unredacted_input": json_body,
                "extraction_method": "http_request_body",
                "note": "This is the actual unredacted data from the HTTP request body"
            },
            "response_status": 200,
            "response_headers": {
                "content-type": "application/json"
            },
            "response_body": null,
            "duration_ms": 0,
            "metadata": {
                "interceptor_name": self.name(),
                "timestamp_iso": chrono::Utc::now().to_rfc3339(),
                "interception_stage": "before_transmit",
                "data_source": "http_request_body_unredacted"
            }
        });

        // Send to debug server asynchronously in a separate thread
        let payload = debug_payload.clone();
        std::thread::spawn(move || {
            // Use a new runtime in the separate thread
            match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(rt) => {
                    rt.block_on(async {
                        let client = reqwest::Client::new();
                        match client
                            .post("http://127.0.0.1:8080/api/requests")
                            .json(&payload)
                            .send()
                            .await
                        {
                            Ok(_response) => {
                                // Success - no output to avoid noise
                            },
                            Err(_e) => {
                                // Failed - no output to avoid noise
                            }
                        }
                    });
                },
                Err(_e) => {
                    // Failed to create runtime - no output to avoid noise
                }
            }
        });
    }
}

impl aws_smithy_runtime_api::client::runtime_plugin::RuntimePlugin for DebugInterceptorPlugin {
    fn config(&self) -> Option<aws_smithy_types::config_bag::FrozenLayer> {
        None
    }

    fn runtime_components(
        &self,
        _: &aws_smithy_runtime_api::client::runtime_components::RuntimeComponentsBuilder,
    ) -> std::borrow::Cow<'_, aws_smithy_runtime_api::client::runtime_components::RuntimeComponentsBuilder> {
        let mut rcb =
            aws_smithy_runtime_api::client::runtime_components::RuntimeComponentsBuilder::new("DebugInterceptorPlugin");
        rcb.push_interceptor(DebugInterceptor);
        std::borrow::Cow::Owned(rcb)
    }
}

/// Create a debug interceptor plugin
pub fn create_debug_interceptor() -> DebugInterceptorPlugin {
    DebugInterceptorPlugin::new()
}
