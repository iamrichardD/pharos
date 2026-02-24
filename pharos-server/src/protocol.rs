/* ========================================================================
 * Project: pharos
 * Component: Server Core
 * File: pharos-server/src/protocol.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This module implements the RFC 2378 command syntax parser. It handles
 * keyword identification, argument splitting, and escape sequence 
 * processing (e.g., \n, \t, \", \\).
 * * Traceability:
 * Implements RFC 2378 Section 2.1 and Appendix C.
 * ======================================================================== */

use thiserror::Error;

#[derive(Debug, PartialEq, Eq)]
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
    Quit,
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
            assert_eq!(force, false);
        } else {
            panic!("Expected Change command");
        }
    }

    #[test]
    fn test_should_return_error_when_quotes_unclosed() {
        assert_eq!(parse_command("query name=\"unclosed"), Err(ProtocolError::SyntaxError));
    }
}
