/* ========================================================================
 * Project: pharos
 * Component: Server Core
 * File: pharos-server/src/middleware.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This module defines the middleware system for pharos-server. It allows
 * for cross-cutting concerns like logging, authentication checks, and
 * request filtering to be implemented as discrete, reusable components.
 * * Traceability:
 * Related to GitHub Issue #33.
 * ======================================================================== */

use crate::protocol::{Command, ProtocolError};
use crate::auth::SecurityTier;
use std::sync::Arc;
use tracing::info;

/// Contextual information about the current client session.
#[derive(Debug, Clone)]
pub struct ClientContext {
    pub id: Option<String>,
    pub authenticated: bool,
    pub peer_addr: String,
    pub roles: Vec<String>,
    pub tier: SecurityTier,
}

impl Default for ClientContext {
    fn default() -> Self {
        Self {
            id: None,
            authenticated: false,
            peer_addr: String::new(),
            roles: Vec::new(),
            tier: SecurityTier::Open,
        }
    }
}

/// The action to take after a middleware's pre-processing.
pub enum MiddlewareAction {
    /// Continue to the next middleware or command execution.
    Continue,
    /// Short-circuit the process and return this response to the client.
    ShortCircuit(String),
}

/// Trait for pharos-server middleware.
pub trait Middleware: Send + Sync {
    /// Executed before the command is processed by the server.
    fn pre_process(&self, _command: &mut Command, _context: &mut ClientContext) -> Result<MiddlewareAction, ProtocolError> {
        Ok(MiddlewareAction::Continue)
    }

    /// Executed after the command has been processed, before the final response is sent.
    /// Note: Currently, the response is written directly to the socket in main.rs.
    /// To support post-processing, we might need to buffer the response or 
    /// pass a callback. For MVP, we'll focus on pre-processing and side-effects.
    fn post_process(&self, _command: &Command, _context: &ClientContext) {}
}

/// A chain of middlewares to be executed in sequence.
pub struct MiddlewareChain {
    middlewares: Vec<Arc<dyn Middleware>>,
}

impl MiddlewareChain {
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }

    pub fn add(&mut self, middleware: Arc<dyn Middleware>) {
        self.middlewares.push(middleware);
    }

    pub fn pre_process(&self, command: &mut Command, context: &mut ClientContext) -> Result<MiddlewareAction, ProtocolError> {
        for middleware in &self.middlewares {
            match middleware.pre_process(command, context)? {
                MiddlewareAction::Continue => continue,
                MiddlewareAction::ShortCircuit(response) => return Ok(MiddlewareAction::ShortCircuit(response)),
            }
        }
        Ok(MiddlewareAction::Continue)
    }

    pub fn post_process(&self, command: &Command, context: &ClientContext) {
        for middleware in &self.middlewares {
            middleware.post_process(command, context);
        }
    }
}

// --- Sample Middlewares ---

/// Simple middleware that logs every command.
pub struct LoggingMiddleware;

impl Middleware for LoggingMiddleware {
    fn pre_process(&self, command: &mut Command, context: &mut ClientContext) -> Result<MiddlewareAction, ProtocolError> {
        info!(peer = %context.peer_addr, client_id = ?context.id, "Processing command: {:?}", command);
        Ok(MiddlewareAction::Continue)
    }
}

/// Middleware that enforces read-only access for specific sessions.
pub struct ReadOnlyMiddleware {
    pub read_only_ids: Vec<String>,
}

impl Middleware for ReadOnlyMiddleware {
    fn pre_process(&self, command: &mut Command, context: &mut ClientContext) -> Result<MiddlewareAction, ProtocolError> {
        let is_write_command = matches!(command, 
            Command::Add(_) | Command::Delete(_) | Command::Change { .. }
        );

        if is_write_command {
            let is_read_only = context.id.as_ref()
                .map(|id| self.read_only_ids.contains(id))
                .unwrap_or(false);

            if is_read_only {
                return Ok(MiddlewareAction::ShortCircuit("500:Read-only access permitted for this ID
".to_string()));
            }
        }

        Ok(MiddlewareAction::Continue)
    }
}

/// Middleware that enforces Triple-Tier Security based on the server's configured tier.
pub struct SecurityTierMiddleware {
    pub default_tier: SecurityTier,
}

impl Middleware for SecurityTierMiddleware {
    fn pre_process(&self, command: &mut Command, context: &mut ClientContext) -> Result<MiddlewareAction, ProtocolError> {
        context.tier = self.default_tier; // Set the tier in the context for other middlewares

        match self.default_tier {
            SecurityTier::Open => {
                // Open tier: Read-only access is open, writes require auth (handled in lib.rs)
                Ok(MiddlewareAction::Continue)
            }
            SecurityTier::Protected => {
                // Protected tier: ALL commands (except auth/status/id/quit) require authentication
                let is_auth_bypassed = matches!(command,
                    Command::Status | Command::Id(_) | Command::Auth { .. } | Command::Quit
                );
                
                if !is_auth_bypassed && !context.authenticated {
                    return Ok(MiddlewareAction::ShortCircuit("401:Authentication required for Protected tier\n".to_string()));
                }
                Ok(MiddlewareAction::Continue)
            }
            SecurityTier::Scoped => {
                // Scoped tier: Same as protected, but also enforces roles
                let is_auth_bypassed = matches!(command,
                    Command::Status | Command::Id(_) | Command::Auth { .. } | Command::Quit
                );
                
                if !is_auth_bypassed && !context.authenticated {
                    return Ok(MiddlewareAction::ShortCircuit("401:Authentication required for Scoped tier\n".to_string()));
                }

                let is_write_command = matches!(command, 
                    Command::Add(_) | Command::Delete(_) | Command::Change { .. }
                );

                if is_write_command && !context.roles.contains(&"admin".to_string()) {
                    return Ok(MiddlewareAction::ShortCircuit("403:Forbidden: Admin role required for write operations\n".to_string()));
                }

                Ok(MiddlewareAction::Continue)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Command;

    #[test]
    fn test_should_continue_when_no_middleware_blocks() {
        let mut chain = MiddlewareChain::new();
        chain.add(Arc::new(LoggingMiddleware));
        
        let mut command = Command::Status;
        let mut context = ClientContext {
            id: Some("test".to_string()),
            authenticated: false,
            peer_addr: "127.0.0.1:1234".to_string(),
            ..Default::default()
        };

        let result = chain.pre_process(&mut command, &mut context).unwrap();
        match result {
            MiddlewareAction::Continue => {},
            _ => panic!("Expected Continue"),
        }
    }

    #[test]
    fn test_should_short_circuit_when_read_only_middleware_blocks_write() {
        let mut chain = MiddlewareChain::new();
        chain.add(Arc::new(ReadOnlyMiddleware {
            read_only_ids: vec!["guest".to_string()],
        }));

        let mut command = Command::Add(vec![("name".to_string(), "Test".to_string())]);
        let mut context = ClientContext {
            id: Some("guest".to_string()),
            authenticated: true,
            peer_addr: "127.0.0.1:1234".to_string(),
            ..Default::default()
        };

        let result = chain.pre_process(&mut command, &mut context).unwrap();
        match result {
            MiddlewareAction::ShortCircuit(resp) => {
                assert!(resp.contains("500:Read-only access"));
            },
            _ => panic!("Expected ShortCircuit"),
        }
    }

    #[test]
    fn test_should_not_block_read_command_in_read_only_middleware() {
        let mut chain = MiddlewareChain::new();
        chain.add(Arc::new(ReadOnlyMiddleware {
            read_only_ids: vec!["guest".to_string()],
        }));

        let mut command = Command::Query { selections: vec![], returns: vec![] };
        let mut context = ClientContext {
            id: Some("guest".to_string()),
            authenticated: true,
            peer_addr: "127.0.0.1:1234".to_string(),
            ..Default::default()
        };

        let result = chain.pre_process(&mut command, &mut context).unwrap();
        match result {
            MiddlewareAction::Continue => {},
            _ => panic!("Expected Continue"),
        }
    }
}
