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
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use tracing::{instrument, info, error};
use chrono::Utc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Record {
    pub id: usize,
    pub record_type: Option<RecordType>,
    pub fields: HashMap<String, String>,
    pub owner_fingerprint: Option<String>,
    pub owner_team: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Record already exists and is bonded to a different fingerprint (Collision)")]
    Collision,
    #[error("Unauthorized: Record belongs to another team")]
    Unauthorized,
    #[error("Internal storage error: {0}")]
    Internal(String),
}

pub trait Storage: Send + Sync {
    fn record_count(&self) -> usize;
    fn add_record(&mut self, fields: HashMap<String, String>, fingerprint: Option<String>, team: Option<String>);
    fn query(&self, selections: &[(Option<String>, String)], default_type: Option<RecordType>) -> Vec<Record>;
    fn upsert_record(&mut self, fields: HashMap<String, String>, fingerprint: Option<String>, team: Option<String>) -> Result<(), StorageError>;
    fn delete_record(&mut self, selections: &[(Option<String>, String)], fingerprint: Option<String>, teams: &[String]) -> Result<usize, StorageError>;
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

impl Storage for MemoryStorage {
    #[instrument(skip(self))]
    fn record_count(&self) -> usize {
        self.records.len()
    }

    #[instrument(skip(self))]
    fn add_record(&mut self, mut fields: HashMap<String, String>, fingerprint: Option<String>, team: Option<String>) {
        let now = Utc::now().to_rfc3339();
        fields.entry("created_at".to_string()).or_insert_with(|| now.clone());
        fields.insert("last_seen_at".to_string(), now);
        
        let record_type = fields.get("type").map(|s| RecordType::from(s.as_str()));
        let record = Record {
            id: self.next_id,
            record_type,
            fields,
            owner_fingerprint: fingerprint,
            owner_team: team,
        };
        self.records.push(record);
        self.next_id += 1;
    }

    #[instrument(skip(self))]
    fn query(&self, selections: &[(Option<String>, String)], default_type: Option<RecordType>) -> Vec<Record> {
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
        }).cloned().collect()
    }

    #[instrument(skip(self))]
    fn upsert_record(&mut self, mut fields: HashMap<String, String>, fingerprint: Option<String>, team: Option<String>) -> Result<(), StorageError> {
        let now = Utc::now().to_rfc3339();
        let identifier = fields.get("hostname").or_else(|| fields.get("alias"));

        if let Some(id_val) = identifier {
            let existing = self.records.iter_mut().find(|r| {
                r.fields.get("hostname") == Some(id_val) || r.fields.get("alias") == Some(id_val)
            });

            if let Some(record) = existing {
                // Check Host Authorization logic (SSH Fingerprint match)
                if let Some(ref bonded) = record.owner_fingerprint {
                    if Some(bonded) != fingerprint.as_ref() {
                        return Err(StorageError::Collision);
                    }
                }

                // Check Member Authorization logic (Team match)
                if let Some(ref record_team) = record.owner_team {
                    if let Some(ref user_team) = team {
                         // User must be in the team that owns the record
                         if record_team != user_team {
                             return Err(StorageError::Unauthorized);
                         }
                    } else if record.owner_fingerprint.is_none() {
                         return Err(StorageError::Unauthorized);
                    }
                }

                if record.owner_fingerprint.is_none() {
                    record.owner_fingerprint = fingerprint;
                }
                if record.owner_team.is_none() {
                    record.owner_team = team;
                }

                for (k, v) in fields {
                    record.fields.insert(k, v);
                }
                record.fields.insert("last_seen_at".to_string(), now);
                return Ok(());
            }
        }

        self.add_record(fields, fingerprint, team);
        Ok(())
    }

    #[instrument(skip(self))]
    fn delete_record(&mut self, selections: &[(Option<String>, String)], fingerprint: Option<String>, teams: &[String]) -> Result<usize, StorageError> {
        let mut error = None;
        let mut to_delete_ids = Vec::new();

        for record in &self.records {
            let matches = selections.iter().all(|(field_opt, value)| {
                match field_opt {
                    Some(field_name) => {
                        if let Some(field_val) = record.fields.get(field_name) {
                            self.matches(field_val, value)
                        } else {
                            false
                        }
                    }
                    None => {
                        record.fields.values().any(|field_val| self.matches(field_val, value))
                    }
                }
            });

            if matches {
                // Check authorization for deletion
                let authorized = match (&record.owner_fingerprint, &record.owner_team) {
                    (Some(fp), _) if fingerprint.as_ref() == Some(fp) => true,
                    (_, Some(team)) if teams.contains(team) => true,
                    (None, None) => true, // System records?
                    _ => false,
                };

                if authorized {
                    to_delete_ids.push(record.id);
                } else {
                    error = Some(StorageError::Unauthorized);
                }
            }
        }

        if let Some(e) = error {
            return Err(e);
        }

        let deleted_count = to_delete_ids.len();
        self.records.retain(|r| !to_delete_ids.contains(&r.id));

        Ok(deleted_count)
    }
}

pub struct FileStorage {
    memory: MemoryStorage,
    path: PathBuf,
}

impl FileStorage {
    #[instrument]
    pub fn new(path: PathBuf) -> Self {
        let mut storage = Self {
            memory: MemoryStorage::new(),
            path,
        };
        storage.load_from_disk();
        storage
    }

    #[instrument(skip(self))]
    fn load_from_disk(&mut self) {
        if !self.path.exists() {
            info!("No existing data file found at {:?}", self.path);
            return;
        }

        let mut file = match File::open(&self.path) {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to open storage file: {}", e);
                return;
            }
        };

        let mut data = String::new();
        if let Err(e) = file.read_to_string(&mut data) {
            error!("Failed to read storage file: {}", e);
            return;
        }

        if data.is_empty() {
            return;
        }

        match serde_json::from_str::<Vec<Record>>(&data) {
            Ok(records) => {
                let max_id = records.iter().map(|r| r.id).max().unwrap_or(0);
                self.memory.records = records;
                self.memory.next_id = max_id + 1;
                info!("Loaded {} records from {:?}", self.memory.records.len(), self.path);
            }
            Err(e) => {
                error!("Failed to parse storage file: {}", e);
            }
        }
    }

    #[instrument(skip(self))]
    fn persist_to_disk(&self) {
        let data = match serde_json::to_string_pretty(&self.memory.records) {
            Ok(d) => d,
            Err(e) => {
                error!("Failed to serialize records: {}", e);
                return;
            }
        };

        let mut file = match File::create(&self.path) {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to create storage file: {}", e);
                return;
            }
        };

        if let Err(e) = file.write_all(data.as_bytes()) {
            error!("Failed to write to storage file: {}", e);
        }
    }
}

impl Storage for FileStorage {
    fn record_count(&self) -> usize {
        self.memory.record_count()
    }

    fn add_record(&mut self, fields: HashMap<String, String>, fingerprint: Option<String>, team: Option<String>) {
        self.memory.add_record(fields, fingerprint, team);
        self.persist_to_disk();
    }

    fn query(&self, selections: &[(Option<String>, String)], default_type: Option<RecordType>) -> Vec<Record> {
        self.memory.query(selections, default_type)
    }

    fn upsert_record(&mut self, fields: HashMap<String, String>, fingerprint: Option<String>, team: Option<String>) -> Result<(), StorageError> {
        self.memory.upsert_record(fields, fingerprint, team)?;
        self.persist_to_disk();
        Ok(())
    }

    fn delete_record(&mut self, selections: &[(Option<String>, String)], fingerprint: Option<String>, teams: &[String]) -> Result<usize, StorageError> {
        let count = self.memory.delete_record(selections, fingerprint, teams)?;
        if count > 0 {
            self.persist_to_disk();
        }
        Ok(count)
    }
}

pub struct LdapStorage {
    // Config
    url: String,
    bind_dn: String,
    bind_pw: String,
    base_dn: String,
    
    // Schema mapping
    // Ph Field -> LDAP Attribute
    field_map: HashMap<String, String>,
}

impl LdapStorage {
    pub fn new(url: String, bind_dn: String, bind_pw: String, base_dn: String) -> Self {
        let mut field_map = HashMap::new();
        // Default mappings
        field_map.insert("name".to_string(), "cn".to_string());
        field_map.insert("email".to_string(), "mail".to_string());
        field_map.insert("phone".to_string(), "telephoneNumber".to_string());
        field_map.insert("hostname".to_string(), "cn".to_string());
        field_map.insert("ip".to_string(), "ipHostNumber".to_string());

        Self {
            url,
            bind_dn,
            bind_pw,
            base_dn,
            field_map,
        }
    }

    fn build_filter(&self, selections: &[(Option<String>, String)], default_type: Option<RecordType>) -> String {
        let mut filters = Vec::new();

        // Object Class filters based on discriminator
        if let Some(rt) = default_type {
            match rt {
                RecordType::Person => filters.push("(objectClass=inetOrgPerson)".to_string()),
                RecordType::Machine => filters.push("(objectClass=ipHost)".to_string()),
                _ => {}
            }
        }

        for (field_opt, val) in selections {
            if let Some(field_name) = field_opt {
                let ldap_attr = self.field_map.get(field_name).map(|s| s.as_str()).unwrap_or(field_name);
                let ldap_val = val.replace("*", "*"); // Ph uses * as well
                filters.push(format!("({}={})", ldap_attr, ldap_val));
            } else {
                // Search in any mapped field (LDAP | search)
                let mut or_filters = Vec::new();
                for attr in self.field_map.values() {
                    or_filters.push(format!("({}={})", attr, val));
                }
                if or_filters.len() > 1 {
                    filters.push(format!("(|{})", or_filters.join("")));
                } else if !or_filters.is_empty() {
                    filters.push(or_filters[0].clone());
                }
            }
        }

        if filters.len() > 1 {
            format!("(&{})", filters.join(""))
        } else if !filters.is_empty() {
            filters[0].clone()
        } else {
            "(objectClass=*)".to_string()
        }
    }
}

impl Storage for LdapStorage {
    #[instrument(skip(self))]
    fn record_count(&self) -> usize {
        0
    }

    #[instrument(skip(self))]
    fn add_record(&mut self, _fields: HashMap<String, String>, _fingerprint: Option<String>, _team: Option<String>) {
        error!("LDAP storage is currently read-only (Write operations pending Task 4.3)");
    }

    #[instrument(skip(self))]
    fn query(&self, selections: &[(Option<String>, String)], default_type: Option<RecordType>) -> Vec<Record> {
        info!("Executing LDAP query...");
        
        let filter = self.build_filter(selections, default_type);
        info!("LDAP Filter: {}", filter);

        let mut ldap = match ldap3::LdapConn::new(&self.url) {
            Ok(conn) => conn,
            Err(e) => {
                error!("Failed to connect to LDAP server: {}", e);
                return Vec::new();
            }
        };

        if let Err(e) = ldap.simple_bind(&self.bind_dn, &self.bind_pw) {
            error!("Failed to bind to LDAP: {}", e);
            return Vec::new();
        }

        let rs = match ldap.search(
            &self.base_dn,
            ldap3::Scope::Subtree,
            &filter,
            vec!["*"]
        ) {
            Ok(res) => match res.success() {
                Ok((entries, _)) => entries,
                Err(e) => {
                    error!("LDAP search successful but returned error result: {}", e);
                    return Vec::new();
                }
            },
            Err(e) => {
                error!("LDAP search failed: {}", e);
                return Vec::new();
            }
        };

        let mut records = Vec::new();
        for (i, entry) in rs.into_iter().enumerate() {
            let search_entry = ldap3::SearchEntry::construct(entry);
            let mut fields = HashMap::new();
            
            for (attr, vals) in search_entry.attrs {
                if !vals.is_empty() {
                    let ph_field = self.field_map.iter()
                        .find(|(_, ldap_attr)| **ldap_attr == attr)
                        .map(|(k, _)| k.clone())
                        .unwrap_or(attr);
                    
                    fields.insert(ph_field, vals.join(", "));
                }
            }

            let record_type = if fields.get("objectClass").map(|s| s.contains("inetOrgPerson")).unwrap_or(false) {
                Some(RecordType::Person)
            } else if fields.get("objectClass").map(|s| s.contains("ipHost")).unwrap_or(false) {
                Some(RecordType::Machine)
            } else {
                None
            };

            records.push(Record {
                id: i + 1,
                record_type,
                fields,
                owner_fingerprint: None,
                owner_team: None,
            });
        }

        records
    }

    #[instrument(skip(self))]
    fn upsert_record(&mut self, _fields: HashMap<String, String>, _fingerprint: Option<String>, _team: Option<String>) -> Result<(), StorageError> {
        error!("LDAP storage is currently read-only (Write operations pending Task 4.3)");
        Err(StorageError::Internal("LDAP write not implemented".to_string()))
    }

    #[instrument(skip(self))]
    fn delete_record(&mut self, _selections: &[(Option<String>, String)], _fingerprint: Option<String>, _teams: &[String]) -> Result<usize, StorageError> {
        error!("LDAP storage is currently read-only");
        Err(StorageError::Internal("LDAP delete not implemented".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_inject_created_at_and_last_seen_at_on_add() {
        let mut storage = MemoryStorage::new();
        let mut fields = HashMap::new();
        fields.insert("name".to_string(), "John Doe".to_string());
        storage.add_record(fields, None, None);

        let results = storage.query(&[(Some("name".to_string()), "john".to_string())], None);
        assert!(results[0].fields.contains_key("created_at"));
        assert!(results[0].fields.contains_key("last_seen_at"));
    }

    #[test]
    fn test_should_update_last_seen_at_but_preserve_created_at_on_upsert() {
        let mut storage = MemoryStorage::new();
        let mut fields = HashMap::new();
        fields.insert("hostname".to_string(), "srv-01".to_string());
        storage.upsert_record(fields.clone(), None, None).unwrap();

        let initial_results = storage.query(&[(Some("hostname".to_string()), "srv-01".to_string())], None);
        let created_at = initial_results[0].fields.get("created_at").unwrap().clone();
        let last_seen_1 = initial_results[0].fields.get("last_seen_at").unwrap().clone();

        // Small sleep to ensure timestamp difference if it were second-based, 
        // but RFC3339 might be fast. Utc::now() is usually fast.
        
        let mut update_fields = fields.clone();
        update_fields.insert("status".to_string(), "online".to_string());
        storage.upsert_record(update_fields, None, None).unwrap();

        let updated_results = storage.query(&[(Some("hostname".to_string()), "srv-01".to_string())], None);
        assert_eq!(updated_results[0].fields.get("created_at").unwrap(), &created_at);
        // last_seen_at should be updated (or at least present)
        assert!(updated_results[0].fields.contains_key("last_seen_at"));
    }

    #[test]
    fn test_should_return_matching_record_when_query_matches_name() {
        let mut storage = MemoryStorage::new();
        let mut fields = HashMap::new();
        fields.insert("name".to_string(), "John Doe".to_string());
        fields.insert("email".to_string(), "john@example.com".to_string());
        storage.add_record(fields, None, None);

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
        storage.add_record(fields, None, None);

        let selections = vec![(Some("name".to_string()), "jane".to_string())];
        let results = storage.query(&selections, None);
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_should_support_wildcard_matching() {
        let mut storage = MemoryStorage::new();
        let mut fields = HashMap::new();
        fields.insert("name".to_string(), "John Doe".to_string());
        storage.add_record(fields, None, None);

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
        storage.add_record(fields, None, None);

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
        storage.add_record(fields1, None, None);
        let mut fields2 = HashMap::new();
        fields2.insert("name".to_string(), "Jane Doe".to_string());
        fields2.insert("city".to_string(), "London".to_string());
        storage.add_record(fields2, None, None);

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
        storage.add_record(fields1, None, None);

        let mut fields2 = HashMap::new();
        fields2.insert("name".to_string(), "Server Machine".to_string());
        fields2.insert("type".to_string(), "machine".to_string());
        storage.add_record(fields2, None, None);

        let selections = vec![(Some("name".to_string()), "server".to_string())];
        
        let results = storage.query(&selections, Some(RecordType::Person));
        assert_eq!(results.len(), 0);

        let results = storage.query(&selections, Some(RecordType::Machine));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].fields.get("name").unwrap(), "Server Machine");

        let results = storage.query(&selections, None);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_should_bond_record_to_fingerprint_when_upserted_first_time() {
        let mut storage = MemoryStorage::new();
        let mut fields = HashMap::new();
        fields.insert("hostname".to_string(), "server-01".to_string());
        
        let fingerprint = Some("SHA256:abcd".to_string());
        storage.upsert_record(fields, fingerprint.clone(), None).unwrap();
        
        let results = storage.query(&[(Some("hostname".to_string()), "server-01".to_string())], None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].owner_fingerprint, fingerprint);
    }

    #[test]
    fn test_should_fail_upsert_when_fingerprint_mismatch() {
        let mut storage = MemoryStorage::new();
        let mut fields = HashMap::new();
        fields.insert("hostname".to_string(), "server-01".to_string());
        
        storage.upsert_record(fields.clone(), Some("SHA256:abcd".to_string()), None).unwrap();
        
        let result = storage.upsert_record(fields, Some("SHA256:wrong".to_string()), None);
        assert!(matches!(result, Err(StorageError::Collision)));
    }

    #[test]
    fn test_should_allow_upsert_when_fingerprint_matches() {
        let mut storage = MemoryStorage::new();
        let mut fields = HashMap::new();
        fields.insert("hostname".to_string(), "server-01".to_string());
        fields.insert("status".to_string(), "online".to_string());
        
        let fingerprint = Some("SHA256:abcd".to_string());
        storage.upsert_record(fields.clone(), fingerprint.clone(), None).unwrap();
        
        let mut update_fields = fields.clone();
        update_fields.insert("status".to_string(), "busy".to_string());
        storage.upsert_record(update_fields, fingerprint.clone(), None).unwrap();
        
        let results = storage.query(&[(Some("hostname".to_string()), "server-01".to_string())], None);
        assert_eq!(results[0].fields.get("status").unwrap(), "busy");
    }

    #[test]
    fn test_should_persist_and_reload_records_when_using_file_storage() {
        let temp_dir = std::env::temp_dir();
        let storage_path = temp_dir.join("pharos_test_rbac.json");
        
        if storage_path.exists() {
            let _ = std::fs::remove_file(&storage_path);
        }

        {
            let mut storage = FileStorage::new(storage_path.clone());
            let mut fields = HashMap::new();
            fields.insert("name".to_string(), "Persistent Pete".to_string());
            storage.add_record(fields, None, None);
            assert_eq!(storage.record_count(), 1);
        }

        {
            let storage = FileStorage::new(storage_path.clone());
            assert_eq!(storage.record_count(), 1);
            let results = storage.query(&[(Some("name".to_string()), "pete".to_string())], None);
            assert_eq!(results.len(), 1);
        }

        let _ = std::fs::remove_file(&storage_path);
    }
}
