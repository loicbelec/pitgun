//! Access Controller for Telemetry Data
//!
//! Provides RDA-like (Rights Data Access) access control for telemetry parameters.
//! Inspired by ECUBridge's access control model.
//!
//! # Access Levels
//!
//! - `Public` - Available to all users
//! - `Protected` - Requires authentication
//! - `Private` - Requires specific role/team membership
//! - `Confidential` - Requires explicit permission grant
//!
//! # Example
//!
//! ```rust,ignore
//! use pitgun_policy::access::{AccessController, AccessLevel, Claims};
//!
//! let mut controller = AccessController::new();
//!
//! // Configure parameter access levels
//! controller.set_access(42, AccessLevel::Private, Some("team_ferrari"));
//! controller.set_access(100, AccessLevel::Public, None);
//!
//! // Check access with JWT claims
//! let claims = Claims::new("user123")
//!     .with_role("engineer")
//!     .with_team("team_ferrari");
//!
//! assert!(controller.check(&claims, 42).is_ok());
//! assert!(controller.check(&claims, 100).is_ok());
//! ```

use pitgun_contract::{ParameterRegistry, TelemetryFrame, TelemetrySample};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Access level for telemetry parameters
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccessLevel {
    /// Available to all users, no authentication required
    Public = 0,
    /// Requires valid authentication token
    Protected = 1,
    /// Requires specific role or team membership
    Private = 2,
    /// Requires explicit permission grant
    Confidential = 3,
}

impl Default for AccessLevel {
    fn default() -> Self {
        Self::Protected
    }
}

impl AccessLevel {
    /// Checks if this level is accessible with the given level
    pub fn is_accessible_by(&self, user_level: AccessLevel) -> bool {
        user_level >= *self
    }
}

/// User claims extracted from JWT or authentication token
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Claims {
    /// User identifier
    pub subject: String,
    /// User's roles
    pub roles: HashSet<String>,
    /// Team memberships
    pub teams: HashSet<String>,
    /// Explicitly granted parameter IDs
    pub granted_parameters: HashSet<u32>,
    /// Maximum access level for this user
    pub max_level: AccessLevel,
    /// Token expiration timestamp (Unix epoch seconds)
    pub expires_at: Option<u64>,
    /// Token issued timestamp
    pub issued_at: Option<u64>,
    /// Token issuer
    pub issuer: Option<String>,
}

impl Claims {
    /// Creates new claims for a user
    pub fn new(subject: impl Into<String>) -> Self {
        Self {
            subject: subject.into(),
            max_level: AccessLevel::Protected,
            ..Default::default()
        }
    }

    /// Creates anonymous claims (public access only)
    pub fn anonymous() -> Self {
        Self {
            subject: "anonymous".into(),
            max_level: AccessLevel::Public,
            ..Default::default()
        }
    }

    /// Creates admin claims (full access)
    pub fn admin(subject: impl Into<String>) -> Self {
        Self {
            subject: subject.into(),
            max_level: AccessLevel::Confidential,
            roles: ["admin".into()].into_iter().collect(),
            ..Default::default()
        }
    }

    /// Adds a role
    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.roles.insert(role.into());
        self
    }

    /// Adds a team membership
    pub fn with_team(mut self, team: impl Into<String>) -> Self {
        self.teams.insert(team.into());
        self
    }

    /// Grants access to a specific parameter
    pub fn with_granted_parameter(mut self, parameter_id: u32) -> Self {
        self.granted_parameters.insert(parameter_id);
        self
    }

    /// Sets the maximum access level
    pub fn with_max_level(mut self, level: AccessLevel) -> Self {
        self.max_level = level;
        self
    }

    /// Sets expiration
    pub fn with_expiry(mut self, expires_at: u64) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Checks if the token is expired
    pub fn is_expired(&self) -> bool {
        if let Some(exp) = self.expires_at {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            now > exp
        } else {
            false
        }
    }

    /// Checks if the user has a specific role
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.contains(role)
    }

    /// Checks if the user is in a specific team
    pub fn in_team(&self, team: &str) -> bool {
        self.teams.contains(team)
    }

    /// Checks if admin
    pub fn is_admin(&self) -> bool {
        self.roles.contains("admin")
    }
}

/// Parameter access configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParameterAccess {
    /// Required access level
    pub level: AccessLevel,
    /// Required team (for Private level)
    pub required_team: Option<String>,
    /// Required role (for Private level)
    pub required_role: Option<String>,
}

impl Default for ParameterAccess {
    fn default() -> Self {
        Self {
            level: AccessLevel::Protected,
            required_team: None,
            required_role: None,
        }
    }
}

/// Access violation details
#[derive(Clone, Debug)]
pub struct AccessViolation {
    /// Parameter ID that was denied
    pub parameter_id: u32,
    /// User who attempted access
    pub subject: String,
    /// Required access level
    pub required_level: AccessLevel,
    /// User's access level
    pub user_level: AccessLevel,
    /// Timestamp of the violation
    pub timestamp: u64,
    /// Reason for denial
    pub reason: String,
}

/// Access check result
pub type AccessResult<T> = Result<T, AccessDenied>;

/// Access denied error
#[derive(Clone, Debug)]
pub struct AccessDenied {
    pub parameter_id: u32,
    pub reason: String,
}

impl std::fmt::Display for AccessDenied {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "access denied to parameter {}: {}",
            self.parameter_id, self.reason
        )
    }
}

impl std::error::Error for AccessDenied {}

/// Audit log entry
#[derive(Clone, Debug, Serialize)]
pub struct AuditLogEntry {
    pub timestamp: u64,
    pub subject: String,
    pub action: String,
    pub parameter_id: u32,
    pub granted: bool,
    pub reason: Option<String>,
}

/// Audit logger trait for extensible logging
pub trait AuditLogger: Send + Sync {
    fn log(&self, entry: AuditLogEntry);
}

/// Default in-memory audit logger
#[derive(Default)]
pub struct InMemoryAuditLog {
    entries: std::sync::Mutex<Vec<AuditLogEntry>>,
    max_entries: usize,
}

impl InMemoryAuditLog {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: std::sync::Mutex::new(Vec::new()),
            max_entries,
        }
    }

    pub fn entries(&self) -> Vec<AuditLogEntry> {
        self.entries.lock().unwrap().clone()
    }

    pub fn violations(&self) -> Vec<AuditLogEntry> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .filter(|e| !e.granted)
            .cloned()
            .collect()
    }
}

impl AuditLogger for InMemoryAuditLog {
    fn log(&self, entry: AuditLogEntry) {
        let mut entries = self.entries.lock().unwrap();
        if entries.len() >= self.max_entries {
            entries.remove(0);
        }
        entries.push(entry);
    }
}

/// Access controller for telemetry parameters
pub struct AccessController {
    /// Default access level for unknown parameters
    default_level: AccessLevel,
    /// Per-parameter access configuration
    parameter_access: HashMap<u32, ParameterAccess>,
    /// Optional parameter registry for metadata lookup
    registry: Option<Arc<ParameterRegistry>>,
    /// Audit logger
    audit_logger: Option<Arc<dyn AuditLogger>>,
    /// Enable audit logging
    audit_enabled: bool,
}

impl Default for AccessController {
    fn default() -> Self {
        Self::new()
    }
}

impl AccessController {
    /// Creates a new access controller with default settings
    pub fn new() -> Self {
        Self {
            default_level: AccessLevel::Protected,
            parameter_access: HashMap::new(),
            registry: None,
            audit_logger: None,
            audit_enabled: false,
        }
    }

    /// Creates an access controller with a parameter registry
    pub fn with_registry(registry: Arc<ParameterRegistry>) -> Self {
        let mut controller = Self::new();
        controller.registry = Some(registry);
        controller
    }

    /// Sets the default access level for unknown parameters
    pub fn set_default_level(&mut self, level: AccessLevel) {
        self.default_level = level;
    }

    /// Enables audit logging
    pub fn enable_audit(&mut self, logger: Arc<dyn AuditLogger>) {
        self.audit_logger = Some(logger);
        self.audit_enabled = true;
    }

    /// Disables audit logging
    pub fn disable_audit(&mut self) {
        self.audit_enabled = false;
    }

    /// Sets access configuration for a parameter
    pub fn set_access(
        &mut self,
        parameter_id: u32,
        level: AccessLevel,
        required_team: Option<String>,
    ) {
        self.parameter_access.insert(
            parameter_id,
            ParameterAccess {
                level,
                required_team,
                required_role: None,
            },
        );
    }

    /// Sets access with role requirement
    pub fn set_access_with_role(
        &mut self,
        parameter_id: u32,
        level: AccessLevel,
        required_role: impl Into<String>,
    ) {
        self.parameter_access.insert(
            parameter_id,
            ParameterAccess {
                level,
                required_team: None,
                required_role: Some(required_role.into()),
            },
        );
    }

    /// Bulk sets access for multiple parameters
    pub fn set_bulk_access(&mut self, parameter_ids: &[u32], level: AccessLevel) {
        for &id in parameter_ids {
            self.set_access(id, level, None);
        }
    }

    /// Gets the access configuration for a parameter
    pub fn get_access(&self, parameter_id: u32) -> ParameterAccess {
        self.parameter_access
            .get(&parameter_id)
            .cloned()
            .unwrap_or(ParameterAccess {
                level: self.default_level,
                required_team: None,
                required_role: None,
            })
    }

    /// Checks if a user can access a parameter
    pub fn check(&self, claims: &Claims, parameter_id: u32) -> AccessResult<()> {
        // Check token expiration
        if claims.is_expired() {
            self.log_access(claims, parameter_id, false, "token expired");
            return Err(AccessDenied {
                parameter_id,
                reason: "token expired".into(),
            });
        }

        // Admins have full access
        if claims.is_admin() {
            self.log_access(claims, parameter_id, true, None);
            return Ok(());
        }

        // Check explicitly granted parameters
        if claims.granted_parameters.contains(&parameter_id) {
            self.log_access(claims, parameter_id, true, None);
            return Ok(());
        }

        let access = self.get_access(parameter_id);

        // Check access level
        if claims.max_level < access.level {
            self.log_access(
                claims,
                parameter_id,
                false,
                &format!(
                    "insufficient level: user={:?}, required={:?}",
                    claims.max_level, access.level
                ),
            );
            return Err(AccessDenied {
                parameter_id,
                reason: format!(
                    "insufficient access level: {:?} < {:?}",
                    claims.max_level, access.level
                ),
            });
        }

        // Check team requirement
        if let Some(ref team) = access.required_team {
            if !claims.in_team(team) {
                self.log_access(
                    claims,
                    parameter_id,
                    false,
                    &format!("not in required team: {}", team),
                );
                return Err(AccessDenied {
                    parameter_id,
                    reason: format!("not in required team: {}", team),
                });
            }
        }

        // Check role requirement
        if let Some(ref role) = access.required_role {
            if !claims.has_role(role) {
                self.log_access(
                    claims,
                    parameter_id,
                    false,
                    &format!("missing required role: {}", role),
                );
                return Err(AccessDenied {
                    parameter_id,
                    reason: format!("missing required role: {}", role),
                });
            }
        }

        self.log_access(claims, parameter_id, true, None);
        Ok(())
    }

    /// Checks access for multiple parameters, returning denied ones
    pub fn check_batch(&self, claims: &Claims, parameter_ids: &[u32]) -> Vec<u32> {
        parameter_ids
            .iter()
            .filter(|&&id| self.check(claims, id).is_err())
            .copied()
            .collect()
    }

    /// Filters a frame to only include accessible samples
    pub fn filter_frame(&self, claims: &Claims, frame: &TelemetryFrame) -> TelemetryFrame {
        let accessible_samples: Vec<TelemetrySample> = frame
            .samples()
            .iter()
            .filter(|s| self.check(claims, s.parameter_id).is_ok())
            .cloned()
            .collect();

        TelemetryFrame::new(
            frame.source_id(),
            frame.sequence(),
            frame.timestamp(),
            accessible_samples,
        )
    }

    /// Logs an access attempt
    fn log_access(&self, claims: &Claims, parameter_id: u32, granted: bool, reason: Option<&str>) {
        if !self.audit_enabled {
            return;
        }

        if let Some(ref logger) = self.audit_logger {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            logger.log(AuditLogEntry {
                timestamp,
                subject: claims.subject.clone(),
                action: "access".into(),
                parameter_id,
                granted,
                reason: reason.map(String::from),
            });
        }
    }

    /// Returns the number of configured parameters
    pub fn len(&self) -> usize {
        self.parameter_access.len()
    }

    /// Checks if no parameters are configured
    pub fn is_empty(&self) -> bool {
        self.parameter_access.is_empty()
    }
}

/// Rate limiter for per-source or per-user rate limiting
#[derive(Clone)]
pub struct RateLimiter {
    /// Maximum requests per window
    max_requests: u64,
    /// Window duration in seconds
    window_secs: u64,
    /// Request counts per key
    counts: std::sync::Arc<std::sync::Mutex<HashMap<String, (u64, u64)>>>,
}

impl RateLimiter {
    /// Creates a new rate limiter
    pub fn new(max_requests: u64, window_secs: u64) -> Self {
        Self {
            max_requests,
            window_secs,
            counts: std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Checks if a request should be allowed
    pub fn check(&self, key: &str) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut counts = self.counts.lock().unwrap();
        let entry = counts.entry(key.to_string()).or_insert((0, now));

        // Reset if window expired
        if now - entry.1 > self.window_secs {
            *entry = (1, now);
            return true;
        }

        // Check limit
        if entry.0 >= self.max_requests {
            return false;
        }

        entry.0 += 1;
        true
    }

    /// Cleans up expired entries
    pub fn cleanup(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut counts = self.counts.lock().unwrap();
        counts.retain(|_, (_, ts)| now - *ts <= self.window_secs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claims_builder() {
        let claims = Claims::new("user123")
            .with_role("engineer")
            .with_team("ferrari")
            .with_max_level(AccessLevel::Private);

        assert_eq!(claims.subject, "user123");
        assert!(claims.has_role("engineer"));
        assert!(claims.in_team("ferrari"));
        assert_eq!(claims.max_level, AccessLevel::Private);
    }

    #[test]
    fn anonymous_access() {
        let claims = Claims::anonymous();
        assert_eq!(claims.max_level, AccessLevel::Public);
    }

    #[test]
    fn admin_access() {
        let claims = Claims::admin("admin");
        assert!(claims.is_admin());
        assert_eq!(claims.max_level, AccessLevel::Confidential);
    }

    #[test]
    fn access_controller_public() {
        let mut controller = AccessController::new();
        controller.set_access(1, AccessLevel::Public, None);

        let claims = Claims::anonymous();
        assert!(controller.check(&claims, 1).is_ok());
    }

    #[test]
    fn access_controller_protected() {
        let mut controller = AccessController::new();
        controller.set_access(1, AccessLevel::Protected, None);

        let anon = Claims::anonymous();
        assert!(controller.check(&anon, 1).is_err());

        let user = Claims::new("user").with_max_level(AccessLevel::Protected);
        assert!(controller.check(&user, 1).is_ok());
    }

    #[test]
    fn access_controller_private_team() {
        let mut controller = AccessController::new();
        controller.set_access(1, AccessLevel::Private, Some("ferrari".into()));

        let wrong_team = Claims::new("user")
            .with_max_level(AccessLevel::Private)
            .with_team("mercedes");
        assert!(controller.check(&wrong_team, 1).is_err());

        let right_team = Claims::new("user")
            .with_max_level(AccessLevel::Private)
            .with_team("ferrari");
        assert!(controller.check(&right_team, 1).is_ok());
    }

    #[test]
    fn access_controller_admin_bypass() {
        let mut controller = AccessController::new();
        controller.set_access(1, AccessLevel::Confidential, Some("secret_team".into()));

        let admin = Claims::admin("admin");
        assert!(controller.check(&admin, 1).is_ok());
    }

    #[test]
    fn access_controller_granted() {
        let mut controller = AccessController::new();
        controller.set_access(1, AccessLevel::Confidential, None);

        let user = Claims::new("user")
            .with_max_level(AccessLevel::Protected)
            .with_granted_parameter(1);
        assert!(controller.check(&user, 1).is_ok());
    }

    #[test]
    fn audit_logging() {
        let mut controller = AccessController::new();
        let logger = Arc::new(InMemoryAuditLog::new(100));
        controller.enable_audit(logger.clone());
        controller.set_access(1, AccessLevel::Protected, None);

        let user = Claims::new("user").with_max_level(AccessLevel::Protected);
        let _ = controller.check(&user, 1);

        let entries = logger.entries();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].granted);
    }

    #[test]
    fn rate_limiter() {
        let limiter = RateLimiter::new(3, 60);

        assert!(limiter.check("user1"));
        assert!(limiter.check("user1"));
        assert!(limiter.check("user1"));
        assert!(!limiter.check("user1")); // Exceeded

        assert!(limiter.check("user2")); // Different key
    }

    #[test]
    fn token_expiry() {
        let claims = Claims::new("user").with_expiry(0); // Expired
        assert!(claims.is_expired());

        let mut controller = AccessController::new();
        controller.set_access(1, AccessLevel::Public, None);
        assert!(controller.check(&claims, 1).is_err());
    }
}
