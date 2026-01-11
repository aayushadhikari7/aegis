//! Network capability for network access.

use serde::{Deserialize, Serialize};

use crate::capability::{
    Action, Capability, CapabilityId, DenialReason, PermissionResult, standard_ids,
};
use crate::error::CapabilityError;

/// Actions related to network operations.
#[derive(Debug, Clone)]
pub enum NetworkAction {
    /// Connect to a host.
    Connect { host: String, port: u16 },
    /// Send data.
    Send { host: String },
    /// Receive data.
    Receive { host: String },
    /// Make an HTTP request.
    HttpRequest { url: String, method: String },
    /// DNS lookup.
    DnsLookup { hostname: String },
}

impl Action for NetworkAction {
    fn action_type(&self) -> &str {
        match self {
            NetworkAction::Connect { .. } => "net:connect",
            NetworkAction::Send { .. } => "net:send",
            NetworkAction::Receive { .. } => "net:receive",
            NetworkAction::HttpRequest { .. } => "net:http",
            NetworkAction::DnsLookup { .. } => "net:dns",
        }
    }

    fn description(&self) -> String {
        match self {
            NetworkAction::Connect { host, port } => format!("Connect to {}:{}", host, port),
            NetworkAction::Send { host } => format!("Send to {}", host),
            NetworkAction::Receive { host } => format!("Receive from {}", host),
            NetworkAction::HttpRequest { url, method } => format!("{} {}", method, url),
            NetworkAction::DnsLookup { hostname } => format!("DNS lookup: {}", hostname),
        }
    }
}

/// Pattern for matching hosts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HostPattern {
    /// Exact host match.
    Exact(String),
    /// Wildcard pattern (e.g., "*.example.com").
    Wildcard(String),
    /// Any host.
    Any,
}

impl HostPattern {
    /// Check if a host matches this pattern.
    pub fn matches(&self, host: &str) -> bool {
        match self {
            HostPattern::Exact(pattern) => pattern == host,
            HostPattern::Wildcard(pattern) => {
                if pattern.starts_with("*.") {
                    let suffix = &pattern[1..]; // Include the dot
                    host.ends_with(suffix) || host == &pattern[2..]
                } else {
                    pattern == host
                }
            }
            HostPattern::Any => true,
        }
    }
}

/// Set of allowed protocols.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolSet {
    /// Allow HTTP.
    pub http: bool,
    /// Allow HTTPS.
    pub https: bool,
    /// Allow raw TCP.
    pub tcp: bool,
    /// Allow UDP.
    pub udp: bool,
}

impl Default for ProtocolSet {
    fn default() -> Self {
        Self {
            http: false,
            https: true, // HTTPS only by default
            tcp: false,
            udp: false,
        }
    }
}

impl ProtocolSet {
    /// Allow all protocols.
    pub fn all() -> Self {
        Self {
            http: true,
            https: true,
            tcp: true,
            udp: true,
        }
    }

    /// Allow only HTTPS.
    pub fn https_only() -> Self {
        Self::default()
    }

    /// Allow HTTP and HTTPS.
    pub fn http_and_https() -> Self {
        Self {
            http: true,
            https: true,
            tcp: false,
            udp: false,
        }
    }
}

/// Capability for network access.
///
/// This capability controls access to network operations, including
/// connections, HTTP requests, and DNS lookups.
///
/// # Example
///
/// ```
/// use aegis_capability::builtin::{NetworkCapability, HostPattern, ProtocolSet};
///
/// // Allow HTTPS connections to example.com and its subdomains
/// let cap = NetworkCapability::new(
///     vec![
///         HostPattern::Exact("example.com".to_string()),
///         HostPattern::Wildcard("*.example.com".to_string()),
///     ],
///     ProtocolSet::https_only(),
/// );
/// ```
#[derive(Debug, Clone)]
pub struct NetworkCapability {
    /// Allowed hosts.
    allowed_hosts: Vec<HostPattern>,
    /// Allowed protocols.
    protocols: ProtocolSet,
    /// Allowed ports (empty means all ports).
    allowed_ports: Vec<u16>,
}

impl NetworkCapability {
    /// Create a new network capability.
    pub fn new(allowed_hosts: Vec<HostPattern>, protocols: ProtocolSet) -> Self {
        Self {
            allowed_hosts,
            protocols,
            allowed_ports: Vec::new(),
        }
    }

    /// Allow connections to any host.
    pub fn allow_all() -> Self {
        Self {
            allowed_hosts: vec![HostPattern::Any],
            protocols: ProtocolSet::all(),
            allowed_ports: Vec::new(),
        }
    }

    /// Allow only HTTPS connections to specific hosts.
    pub fn https_only(hosts: Vec<String>) -> Self {
        Self {
            allowed_hosts: hosts.into_iter().map(HostPattern::Exact).collect(),
            protocols: ProtocolSet::https_only(),
            allowed_ports: vec![443],
        }
    }

    /// Set allowed ports.
    pub fn with_ports(mut self, ports: Vec<u16>) -> Self {
        self.allowed_ports = ports;
        self
    }

    /// Check if a host is allowed.
    pub fn is_host_allowed(&self, host: &str) -> bool {
        self.allowed_hosts.iter().any(|p| p.matches(host))
    }

    /// Check if a port is allowed.
    pub fn is_port_allowed(&self, port: u16) -> bool {
        self.allowed_ports.is_empty() || self.allowed_ports.contains(&port)
    }
}

impl Capability for NetworkCapability {
    fn id(&self) -> CapabilityId {
        standard_ids::NETWORK.clone()
    }

    fn name(&self) -> &str {
        "Network"
    }

    fn description(&self) -> &str {
        "Allows network access"
    }

    fn permits(&self, action: &dyn Action) -> PermissionResult {
        let action_type = action.action_type();
        if !action_type.starts_with("net:") {
            return PermissionResult::NotApplicable;
        }

        // For proper implementation, we'd need to downcast the action
        // Here we return NotApplicable as a placeholder
        PermissionResult::NotApplicable
    }

    fn handled_action_types(&self) -> Vec<&'static str> {
        vec![
            "net:connect",
            "net:send",
            "net:receive",
            "net:http",
            "net:dns",
        ]
    }

    fn validate(&self) -> Result<(), CapabilityError> {
        if self.allowed_hosts.is_empty() {
            return Err(CapabilityError::InvalidConfig(
                "Network capability has no allowed hosts".to_string(),
            ));
        }
        Ok(())
    }
}

/// Helper function to check network permission with a concrete action.
pub fn check_network_permission(
    capability: &NetworkCapability,
    action: &NetworkAction,
) -> PermissionResult {
    match action {
        NetworkAction::Connect { host, port } => {
            if !capability.is_host_allowed(host) {
                return PermissionResult::Denied(DenialReason::new(
                    capability.id(),
                    action.action_type(),
                    format!("Host not allowed: {}", host),
                ));
            }
            if !capability.is_port_allowed(*port) {
                return PermissionResult::Denied(DenialReason::new(
                    capability.id(),
                    action.action_type(),
                    format!("Port not allowed: {}", port),
                ));
            }
            PermissionResult::Allowed
        }
        NetworkAction::HttpRequest { url, .. } => {
            // Extract host from URL
            if let Some(host) = extract_host_from_url(url) {
                if !capability.is_host_allowed(&host) {
                    return PermissionResult::Denied(DenialReason::new(
                        capability.id(),
                        action.action_type(),
                        format!("Host not allowed: {}", host),
                    ));
                }
                // Check protocol
                if url.starts_with("http://") && !capability.protocols.http {
                    return PermissionResult::Denied(DenialReason::new(
                        capability.id(),
                        action.action_type(),
                        "HTTP not allowed",
                    ));
                }
                if url.starts_with("https://") && !capability.protocols.https {
                    return PermissionResult::Denied(DenialReason::new(
                        capability.id(),
                        action.action_type(),
                        "HTTPS not allowed",
                    ));
                }
                PermissionResult::Allowed
            } else {
                PermissionResult::Denied(DenialReason::new(
                    capability.id(),
                    action.action_type(),
                    "Invalid URL",
                ))
            }
        }
        NetworkAction::DnsLookup { hostname } => {
            if capability.is_host_allowed(hostname) {
                PermissionResult::Allowed
            } else {
                PermissionResult::Denied(DenialReason::new(
                    capability.id(),
                    action.action_type(),
                    format!("DNS lookup not allowed for: {}", hostname),
                ))
            }
        }
        NetworkAction::Send { host } | NetworkAction::Receive { host } => {
            if capability.is_host_allowed(host) {
                PermissionResult::Allowed
            } else {
                PermissionResult::Denied(DenialReason::new(
                    capability.id(),
                    action.action_type(),
                    format!("Host not allowed: {}", host),
                ))
            }
        }
    }
}

fn extract_host_from_url(url: &str) -> Option<String> {
    let url = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host = url.split('/').next()?;
    let host = host.split(':').next()?;
    Some(host.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_pattern_exact() {
        let pattern = HostPattern::Exact("example.com".to_string());
        assert!(pattern.matches("example.com"));
        assert!(!pattern.matches("sub.example.com"));
        assert!(!pattern.matches("other.com"));
    }

    #[test]
    fn test_host_pattern_wildcard() {
        let pattern = HostPattern::Wildcard("*.example.com".to_string());
        assert!(pattern.matches("sub.example.com"));
        assert!(pattern.matches("deep.sub.example.com"));
        assert!(pattern.matches("example.com")); // Base domain matches too
        assert!(!pattern.matches("other.com"));
    }

    #[test]
    fn test_host_pattern_any() {
        let pattern = HostPattern::Any;
        assert!(pattern.matches("anything.com"));
        assert!(pattern.matches("192.168.1.1"));
    }

    #[test]
    fn test_network_capability_https_only() {
        let cap = NetworkCapability::https_only(vec!["api.example.com".to_string()]);

        assert!(cap.is_host_allowed("api.example.com"));
        assert!(!cap.is_host_allowed("other.com"));
        assert!(cap.is_port_allowed(443));
        assert!(!cap.is_port_allowed(80));
    }

    #[test]
    fn test_check_network_permission() {
        let cap = NetworkCapability::new(
            vec![HostPattern::Exact("api.example.com".to_string())],
            ProtocolSet::http_and_https(),
        );

        let allowed = NetworkAction::HttpRequest {
            url: "https://api.example.com/data".to_string(),
            method: "GET".to_string(),
        };
        assert!(check_network_permission(&cap, &allowed).is_allowed());

        let denied = NetworkAction::HttpRequest {
            url: "https://evil.com/data".to_string(),
            method: "GET".to_string(),
        };
        assert!(check_network_permission(&cap, &denied).is_denied());
    }

    #[test]
    fn test_extract_host_from_url() {
        assert_eq!(
            extract_host_from_url("https://example.com/path"),
            Some("example.com".to_string())
        );
        assert_eq!(
            extract_host_from_url("http://api.example.com:8080/path"),
            Some("api.example.com".to_string())
        );
    }
}
