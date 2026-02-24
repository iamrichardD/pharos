/* ========================================================================
 * Project: pharos
 * Component: Server Core
 * File: pharos-server/src/storage.rs
 * Author: Richard D. (https://github.com/iamrichardd)
 * License: AGPL-3.0 (See LICENSE file for details)
 * * Purpose (The "Why"):
 * This module implements the in-memory storage engine for the Ph protocol.
 * It provides the core data structures for records and fields, along with
 * search logic optimized for read-heavy workloads.
 * * Traceability:
 * Implements RFC 2378 Section 1.1 and Section 3.
 * ======================================================================== */

use std::collections::HashMap;
use tracing::instrument;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordType {
    Person,
    Machine,
    Other(String),
}

impl From<&str> for RecordType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "person" => RecordType::Person,
            "machine" => RecordType::Machine,
            _ => RecordType::Other(s.to_string()),
        }
    }
}

impl RecordType {
    pub fn as_str(&self) -> &str {
        match self {
            RecordType::Person => "person",
            RecordType::Machine => "machine",
            RecordType::Other(s) => s.as_str(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Record {
    pub id: usize,
    pub record_type: Option<RecordType>,
    pub fields: HashMap<String, String>,
}

pub struct MemoryStorage {
    records: Vec<Record>,
    next_id: usize,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            next_id: 1,
        }
    }

    #[instrument(skip(self))]
    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    #[instrument(skip(self))]
    pub fn add_record(&mut self, fields: HashMap<String, String>) {
        let record_type = fields.get("type").map(|s| RecordType::from(s.as_str()));
        let record = Record {
            id: self.next_id,
            record_type,
            fields,
        };
        self.records.push(record);
        self.next_id += 1;
    }

    #[instrument(skip(self))]
    pub fn query(&self, selections: &[(Option<String>, String)], default_type: Option<RecordType>) -> Vec<&Record> {
        self.records.iter().filter(|record| {
            // Check discriminator
            if let Some(ref dt) = default_type {
                if let Some(ref rt) = record.record_type {
                    if rt != dt && !selections.iter().any(|(f, _)| f.as_deref() == Some("type")) {
                        return false;
                    }
                } else {
                    return false;
                }
            }

            selections.iter().all(|(field_opt, value)| {
                match field_opt {
                    Some(field_name) => {
                        // Exact or wildcard match on specific field
                        if let Some(field_val) = record.fields.get(field_name) {
                            self.matches(field_val, value)
                        } else {
                            false
                        }
                    }
                    None => {
                        // Search in any "searchable" field (for MVP, all fields)
                        record.fields.values().any(|field_val| self.matches(field_val, value))
                    }
                }
            })
        }).collect()
    }

    fn matches(&self, field_val: &str, query_val: &str) -> bool {
        let field_val_lower = field_val.to_lowercase();
        let query_val_lower = query_val.to_lowercase();

        // Simple word-based matching for MVP
        // RFC 2378 says "normally done on a word-by-word basis"
        let query_words: Vec<&str> = query_val_lower.split_whitespace().collect();
        let field_words: Vec<&str> = field_val_lower.split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == ':').collect();

        query_words.iter().all(|&qw| {
            field_words.iter().any(|&fw| {
                if qw.contains('*') || qw.contains('?') || qw.contains('+') {
                    self.wildcard_match(fw, qw)
                } else {
                    fw == qw
                }
            })
        })
    }

    fn wildcard_match(&self, word: &str, pattern: &str) -> bool {
        // Very basic wildcard support for MVP: '*' only at the end
        if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len() - 1];
            word.starts_with(prefix)
        } else {
            // Fallback to exact match for unsupported wildcards
            word == pattern
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_return_matching_record_when_query_matches_name() {
        let mut storage = MemoryStorage::new();
        let mut fields = HashMap::new();
        fields.insert("name".to_string(), "John Doe".to_string());
        fields.insert("email".to_string(), "john@example.com".to_string());
        storage.add_record(fields);

        let selections = vec![(Some("name".to_string()), "john".to_string())];
        let results = storage.query(&selections, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].fields.get("email").unwrap(), "john@example.com");
    }

    #[test]
    fn test_should_return_empty_when_query_does_not_match() {
        let mut storage = MemoryStorage::new();
        let mut fields = HashMap::new();
        fields.insert("name".to_string(), "John Doe".to_string());
        storage.add_record(fields);

        let selections = vec![(Some("name".to_string()), "jane".to_string())];
        let results = storage.query(&selections, None);
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_should_support_wildcard_matching() {
        let mut storage = MemoryStorage::new();
        let mut fields = HashMap::new();
        fields.insert("name".to_string(), "John Doe".to_string());
        storage.add_record(fields);

        let selections = vec![(Some("name".to_string()), "jo*".to_string())];
        let results = storage.query(&selections, None);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_should_match_any_field_when_no_field_name_provided() {
        let mut storage = MemoryStorage::new();
        let mut fields = HashMap::new();
        fields.insert("name".to_string(), "John Doe".to_string());
        fields.insert("alias".to_string(), "jdoe".to_string());
        storage.add_record(fields);

        let selections = vec![(None, "jdoe".to_string())];
        let results = storage.query(&selections, None);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_should_match_multiple_criteria_with_implicit_and() {
        let mut storage = MemoryStorage::new();
        let mut fields1 = HashMap::new();
        fields1.insert("name".to_string(), "John Doe".to_string());
        fields1.insert("city".to_string(), "New York".to_string());
        storage.add_record(fields1);

        let mut fields2 = HashMap::new();
        fields2.insert("name".to_string(), "Jane Doe".to_string());
        fields2.insert("city".to_string(), "London".to_string());
        storage.add_record(fields2);

        let selections = vec![
            (Some("name".to_string()), "doe".to_string()),
            (Some("city".to_string()), "london".to_string()),
        ];
        let results = storage.query(&selections, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].fields.get("name").unwrap(), "Jane Doe");
    }

    #[test]
    fn test_should_filter_by_type_discriminator() {
        let mut storage = MemoryStorage::new();
        
        let mut fields1 = HashMap::new();
        fields1.insert("name".to_string(), "John Person".to_string());
        fields1.insert("type".to_string(), "person".to_string());
        storage.add_record(fields1);

        let mut fields2 = HashMap::new();
        fields2.insert("name".to_string(), "Server Machine".to_string());
        fields2.insert("type".to_string(), "machine".to_string());
        storage.add_record(fields2);

        let selections = vec![(Some("name".to_string()), "server".to_string())];
        
        // Query as Person (should not find the machine)
        let results = storage.query(&selections, Some(RecordType::Person));
        assert_eq!(results.len(), 0);

        // Query as Machine (should find the machine)
        let results = storage.query(&selections, Some(RecordType::Machine));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].fields.get("name").unwrap(), "Server Machine");

        // Query without discriminator (should find the machine)
        let results = storage.query(&selections, None);
        assert_eq!(results.len(), 1);
    }
}
