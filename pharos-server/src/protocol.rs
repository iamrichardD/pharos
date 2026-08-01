/* ========================================================================
 * Project: pharos
 * Component: Server Core
 * File: pharos-server/src/protocol.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This module implements the RFC 2378 command syntax parser. It handles
 * keyword identification, argument splitting, escape sequence 
 * processing (e.g., \n, \t, \", \\), and custom extensions like 'auth'.
 * * Traceability:
 * Implements RFC 2378 Section 2.1 and Appendix C.
 * ======================================================================== */

use thiserror::Error;

#[derive(PartialEq, Eq)]
pub enum Command {
    Status,
    SiteInfo,
    Fields(Vec<String>),
    Id(String),
    Set(Vec<String>),
    Login(String),
    Logout,
    Answer(String),
    Clear(String),
    Email(String),
    XLogin(u32, String),
    Add(Vec<(String, String)>),
    Query {
        selections: Vec<(Option<String>, String)>,
        returns: Vec<String>,
    },
    Delete(Vec<(Option<String>, String)>),
    Change {
        selections: Vec<(Option<String>, String)>,
        modifications: Vec<(String, String)>,
        force: bool,
    },
    Help {
        target: Option<String>,
        topics: Vec<String>,
    },
    Auth {
        public_key: String,
        signature: String,
    },
    AuthCheck {
        public_key: String,
        signature: String,
        challenge: String,
    },
    Quit,
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Command::Status => write!(f, "Status"),
            Command::SiteInfo => write!(f, "SiteInfo"),
            Command::Fields(v) => f.debug_tuple("Fields").field(v).finish(),
            Command::Id(v) => f.debug_tuple("Id").field(v).finish(),
            Command::Set(v) => f.debug_tuple("Set").field(v).finish(),
            Command::Login(v) => f.debug_tuple("Login").field(v).finish(),
            Command::Logout => write!(f, "Logout"),
            Command::Answer(v) => f.debug_tuple("Answer").field(v).finish(),
            Command::Clear(v) => f.debug_tuple("Clear").field(v).finish(),
            Command::Email(v) => f.debug_tuple("Email").field(v).finish(),
            Command::XLogin(a, b) => f.debug_tuple("XLogin").field(a).field(b).finish(),
            Command::Add(v) => f.debug_tuple("Add").field(v).finish(),
            Command::Query { selections, returns } => f
                .debug_struct("Query")
                .field("selections", selections)
                .field("returns", returns)
                .finish(),
            Command::Delete(v) => f.debug_tuple("Delete").field(v).finish(),
            Command::Change { selections, modifications, force } => f
                .debug_struct("Change")
                .field("selections", selections)
                .field("modifications", modifications)
                .field("force", force)
                .finish(),
            Command::Help { target, topics } => f
                .debug_struct("Help")
                .field("target", target)
                .field("topics", topics)
                .finish(),
            Command::Auth { public_key, signature: _ } => f
                .debug_struct("Auth")
                .field("public_key", public_key)
                .field("signature", &"<redacted>")
                .finish(),
            Command::AuthCheck { public_key, signature: _, challenge: _ } => f
                .debug_struct("AuthCheck")
                .field("public_key", public_key)
                .field("signature", &"<redacted>")
                .field("challenge", &"<redacted>")
                .finish(),
            Command::Quit => write!(f, "Quit"),
        }
    }
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("Unknown command")]
    UnknownCommand,
    #[error("Syntax error")]
    SyntaxError,
    #[error("Invalid argument")]
    InvalidArgument,
}

pub fn parse_command(line: &str) -> Result<Command, ProtocolError> {
    let tokens = tokenize(line)?;
    if tokens.is_empty() {
        return Err(ProtocolError::SyntaxError);
    }

    let keyword = tokens[0].to_lowercase();
    match keyword.as_str() {
        "status" => Ok(Command::Status),
        "siteinfo" => Ok(Command::SiteInfo),
        "fields" => Ok(Command::Fields(tokens[1..].to_vec())),
        "id" => {
            if tokens.len() < 2 {
                return Err(ProtocolError::SyntaxError);
            }
            Ok(Command::Id(tokens[1..].join(" ")))
        }
        "auth" => {
            if tokens.len() < 3 {
                return Err(ProtocolError::SyntaxError);
            }
            Ok(Command::Auth {
                public_key: tokens[1].clone(),
                signature: tokens[2].clone(),
            })
        }
        "auth-check" => {
            if tokens.len() < 4 {
                return Err(ProtocolError::SyntaxError);
            }
            Ok(Command::AuthCheck {
                public_key: tokens[1].clone(),
                signature: tokens[2].clone(),
                challenge: tokens[3].clone(),
            })
        }
        "set" => Ok(Command::Set(tokens[1..].to_vec())),
        "login" => {
            if tokens.len() < 2 {
                return Err(ProtocolError::SyntaxError);
            }
            Ok(Command::Login(tokens[1].clone()))
        }
        "logout" => Ok(Command::Logout),
        "answer" => {
            if tokens.len() < 2 {
                return Err(ProtocolError::SyntaxError);
            }
            Ok(Command::Answer(tokens[1].clone()))
        }
        "clear" => {
            if tokens.len() < 2 {
                return Err(ProtocolError::SyntaxError);
            }
            Ok(Command::Clear(tokens[1].clone()))
        }
        "email" => {
            if tokens.len() < 2 {
                return Err(ProtocolError::SyntaxError);
            }
            Ok(Command::Email(tokens[1].clone()))
        }
        "xlogin" => {
            if tokens.len() < 3 {
                return Err(ProtocolError::SyntaxError);
            }
            let option = tokens[1].parse::<u32>().map_err(|_| ProtocolError::InvalidArgument)?;
            Ok(Command::XLogin(option, tokens[2].clone()))
        }
        "add" => {
            let mut pairs = Vec::new();
            for token in &tokens[1..] {
                if let Some((k, v)) = parse_attr_value(token) {
                    pairs.push((k, v));
                } else {
                    return Err(ProtocolError::SyntaxError);
                }
            }
            Ok(Command::Add(pairs))
        }
        "query" | "ph" => {
            let mut selections = Vec::new();
            let mut returns = Vec::new();
            let mut in_returns = false;

            for token in &tokens[1..] {
                if token.to_lowercase() == "return" {
                    in_returns = true;
                    continue;
                }

                if in_returns {
                    returns.push(token.clone());
                } else {
                    if let Some((k, v)) = parse_attr_value(token) {
                        selections.push((Some(k), v));
                    } else {
                        selections.push((None, token.clone()));
                    }
                }
            }
            Ok(Command::Query { selections, returns })
        }
        "delete" => {
            let mut selections = Vec::new();
            for token in &tokens[1..] {
                if let Some((k, v)) = parse_attr_value(token) {
                    selections.push((Some(k), v));
                } else {
                    selections.push((None, token.clone()));
                }
            }
            Ok(Command::Delete(selections))
        }
        "change" => {
            let mut selections = Vec::new();
            let mut modifications = Vec::new();
            let mut force = false;
            let mut phase = 0; // 0: selection, 1: make/force

            for token in &tokens[1..] {
                let lower = token.to_lowercase();
                if lower == "make" || lower == "force" {
                    force = lower == "force";
                    phase = 1;
                    continue;
                }

                if phase == 0 {
                    if let Some((k, v)) = parse_attr_value(token) {
                        selections.push((Some(k), v));
                    } else {
                        selections.push((None, token.clone()));
                    }
                } else {
                    if let Some((k, v)) = parse_attr_value(token) {
                        modifications.push((k, v));
                    } else {
                        return Err(ProtocolError::SyntaxError);
                    }
                }
            }
            Ok(Command::Change { selections, modifications, force })
        }
        "help" => {
            let mut target = None;
            let mut topics = Vec::new();
            if tokens.len() > 1 {
                let first = tokens[1].to_lowercase();
                if first == "native" || first == "ph" { // simplified for now
                    target = Some(first);
                    topics.extend(tokens[2..].iter().cloned());
                } else {
                    topics.extend(tokens[1..].iter().cloned());
                }
            }
            Ok(Command::Help { target, topics })
        }
        "quit" | "exit" | "stop" => Ok(Command::Quit),
        _ => Err(ProtocolError::UnknownCommand),
    }
}

fn parse_attr_value(token: &str) -> Option<(String, String)> {
    if let Some(pos) = token.find('=') {
        let key = token[..pos].to_string();
        let value = token[pos + 1..].to_string();
        Some((key, value))
    } else {
        None
    }
}

fn tokenize(line: &str) -> Result<Vec<String>, ProtocolError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut escaped = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if escaped {
            match c {
                'n' => current.push('\n'),
                't' => current.push('\t'),
                '"' => current.push('"'),
                '\\' => current.push('\\'),
                _ => current.push(c),
            }
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            in_quotes = !in_quotes;
        } else if c.is_whitespace() && !in_quotes {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
        } else {
            current.push(c);
        }
        i += 1;
    }

    if in_quotes {
        return Err(ProtocolError::SyntaxError);
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    Ok(tokens)
}

/// Redacts the argument portion of `auth`/`auth-check` wire lines for logging, so raw signature
/// and challenge material is never written to logs verbatim. Uses a cheap first-word check
/// (not full tokenization) so a malformed line (e.g. unclosed quotes) can never bypass redaction
/// by failing to tokenize — this must be infallible.
pub fn redact_wire_line_for_logging(line: &str) -> String {
    let first_word = line.split_whitespace().next().unwrap_or("").to_lowercase();
    match first_word.as_str() {
        "auth" | "auth-check" => format!("{} <redacted>", first_word),
        _ => line.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_parse_status_when_status_sent() {
        assert_eq!(parse_command("status"), Ok(Command::Status));
        assert_eq!(parse_command("STATUS"), Ok(Command::Status));
    }

    #[test]
    fn test_should_parse_query_with_quotes_and_escapes() {
        let cmd = parse_command("query name=\"John \\\"Doe\\\"\" return email").unwrap();
        if let Command::Query { selections, returns } = cmd {
            assert_eq!(selections, vec![(Some("name".to_string()), "John \"Doe\"".to_string())]);
            assert_eq!(returns, vec!["email".to_string()]);
        } else {
            panic!("Expected Query command");
        }
    }

    #[test]
    fn test_should_parse_change_command() {
        let cmd = parse_command("change alias=j-doe make fax=\"555-1212\"").unwrap();
        if let Command::Change { selections, modifications, force } = cmd {
            assert_eq!(selections, vec![(Some("alias".to_string()), "j-doe".to_string())]);
            assert_eq!(modifications, vec![("fax".to_string(), "555-1212".to_string())]);
            assert!(!force);
        } else {
            panic!("Expected Change command");
        }
    }

    #[test]
    fn test_should_return_error_when_quotes_unclosed() {
        assert_eq!(parse_command("query name=\"unclosed"), Err(ProtocolError::SyntaxError));
    }

    #[test]
    fn test_should_parse_auth_command() {
        let cmd = parse_command("auth mypubkey123 mysignature456").unwrap();
        assert_eq!(cmd, Command::Auth {
            public_key: "mypubkey123".to_string(),
            signature: "mysignature456".to_string(),
        });
    }

    #[test]
    fn test_should_parse_auth_check_command() {
        let cmd = parse_command("auth-check mypubkey123 mysignature456 mychallenge789").unwrap();
        assert_eq!(cmd, Command::AuthCheck {
            public_key: "mypubkey123".to_string(),
            signature: "mysignature456".to_string(),
            challenge: "mychallenge789".to_string(),
        });
    }

    #[test]
    fn test_should_redact_signature_from_auth_debug_output() {
        let cmd = Command::Auth {
            public_key: "mypubkey123".to_string(),
            signature: "TOP-SECRET-SIGNATURE".to_string(),
        };
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("mypubkey123"), "public_key should remain visible: {debug_str}");
        assert!(!debug_str.contains("TOP-SECRET-SIGNATURE"), "signature must not appear in Debug output: {debug_str}");
        assert!(debug_str.contains("<redacted>"), "expected a redaction marker: {debug_str}");
    }

    #[test]
    fn test_should_redact_signature_and_challenge_from_auth_check_debug_output() {
        let cmd = Command::AuthCheck {
            public_key: "mypubkey123".to_string(),
            signature: "TOP-SECRET-SIGNATURE".to_string(),
            challenge: "TOP-SECRET-CHALLENGE".to_string(),
        };
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("mypubkey123"), "public_key should remain visible: {debug_str}");
        assert!(!debug_str.contains("TOP-SECRET-SIGNATURE"), "signature must not appear in Debug output: {debug_str}");
        assert!(!debug_str.contains("TOP-SECRET-CHALLENGE"), "challenge must not appear in Debug output: {debug_str}");
    }

    #[test]
    fn test_should_not_redact_non_auth_command_debug_output() {
        let cmd = Command::Add(vec![("name".to_string(), "Jane Smith".to_string())]);
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("Jane Smith"), "Add field values must NOT be redacted (out of scope for #161): {debug_str}");
    }

    #[test]
    fn test_should_redact_auth_wire_line() {
        let redacted = redact_wire_line_for_logging("auth mypubkey123 TOP-SECRET-SIGNATURE");
        assert!(!redacted.contains("TOP-SECRET-SIGNATURE"), "raw signature must not appear: {redacted}");
        assert!(!redacted.contains("mypubkey123"), "public_key is also dropped for this raw-line case: {redacted}");
        assert_eq!(redacted, "auth <redacted>");
    }

    #[test]
    fn test_should_redact_auth_check_wire_line() {
        let redacted = redact_wire_line_for_logging("auth-check mypubkey123 TOP-SECRET-SIG TOP-SECRET-CHALLENGE");
        assert!(!redacted.contains("TOP-SECRET-SIG"), "raw signature must not appear: {redacted}");
        assert!(!redacted.contains("TOP-SECRET-CHALLENGE"), "raw challenge must not appear: {redacted}");
        assert_eq!(redacted, "auth-check <redacted>");
    }

    #[test]
    fn test_should_redact_auth_wire_line_case_insensitively() {
        let redacted = redact_wire_line_for_logging("AUTH mypubkey123 TOP-SECRET-SIGNATURE");
        assert!(!redacted.contains("TOP-SECRET-SIGNATURE"), "raw signature must not appear regardless of case: {redacted}");
    }

    #[test]
    fn test_should_not_redact_non_auth_wire_line() {
        let line = "add name=\"Jane Smith\" mail=\"jane@example.com\"";
        let redacted = redact_wire_line_for_logging(line);
        assert_eq!(redacted, line, "non-auth wire lines must be logged unchanged");
    }

    #[test]
    fn test_should_handle_malformed_auth_wire_line_without_panicking() {
        // Unclosed quote / garbage input must still redact cleanly, not panic, not leak.
        let redacted = redact_wire_line_for_logging("auth-check \"unclosed TOP-SECRET-CHALLENGE");
        assert!(!redacted.contains("TOP-SECRET-CHALLENGE"), "must redact even on malformed input: {redacted}");
    }

    #[test]
    fn test_should_handle_empty_line_without_panicking() {
        let redacted = redact_wire_line_for_logging("");
        assert_eq!(redacted, "");
    }
}
