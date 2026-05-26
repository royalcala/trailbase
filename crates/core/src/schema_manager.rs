/// Schema management: Initialize, validate, and sync database schemas
/// 
/// This module automates:
/// - Schema structure validation (system/ vs app/)
/// - Database initialization from schema files
/// - Detection of system/ changes
/// - Automatic migration generation and application
/// - Multi-org schema application

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use chrono::Utc;

/// Schema directory structure
#[derive(Debug, Clone)]
pub struct SchemaStructure {
    pub base_dir: PathBuf,
    pub system_main: PathBuf,
    pub system_org: PathBuf,
    pub app_main: PathBuf,
    pub app_org: PathBuf,
    pub migrations_main: PathBuf,
    pub migrations_org: PathBuf,
}

impl SchemaStructure {
    /// Detect and validate schema directory structure
    pub fn detect(base_dir: &Path) -> Result<Self, String> {
        let schema_dir = base_dir.join("schema");
        
        if !schema_dir.is_dir() {
            return Err(format!("Schema directory not found: {:?}", schema_dir));
        }

        let system_dir = schema_dir.join("system");
        let app_dir = schema_dir.join("app");
        let migrations_dir = base_dir.join("migrations");

        // Validate directories
        if !system_dir.is_dir() {
            return Err(format!("system/ directory not found: {:?}", system_dir));
        }
        if !app_dir.is_dir() {
            return Err(format!("app/ directory not found: {:?}", app_dir));
        }

        Ok(SchemaStructure {
            base_dir: base_dir.to_path_buf(),
            system_main: system_dir.join("main.sql"),
            system_org: system_dir.join("org.sql"),
            app_main: app_dir.join("main.sql"),
            app_org: app_dir.join("org.sql"),
            migrations_main: migrations_dir.join("main"),
            migrations_org: migrations_dir.join("orgs"),
        })
    }

    /// Validate that schema files exist and are readable
    pub fn validate(&self) -> Result<(), String> {
        for (name, path) in [
            ("system/main.sql", &self.system_main),
            ("system/org.sql", &self.system_org),
            ("app/main.sql", &self.app_main),
            ("app/org.sql", &self.app_org),
        ] {
            if !path.is_file() {
                return Err(format!("Schema file not found: {}", name));
            }

            // Try reading to catch encoding issues
            fs::read_to_string(path)
                .map_err(|e| format!("Failed to read {}: {}", name, e))?;
        }

        Ok(())
    }

    /// Get content of system/main.sql
    pub fn read_system_main(&self) -> Result<String, String> {
        fs::read_to_string(&self.system_main)
            .map_err(|e| format!("Failed to read system/main.sql: {}", e))
    }

    /// Get content of app/main.sql
    pub fn read_app_main(&self) -> Result<String, String> {
        fs::read_to_string(&self.app_main)
            .map_err(|e| format!("Failed to read app/main.sql: {}", e))
    }

    /// Get content of system/org.sql
    pub fn read_system_org(&self) -> Result<String, String> {
        fs::read_to_string(&self.system_org)
            .map_err(|e| format!("Failed to read system/org.sql: {}", e))
    }

    /// Get content of app/org.sql
    pub fn read_app_org(&self) -> Result<String, String> {
        fs::read_to_string(&self.app_org)
            .map_err(|e| format!("Failed to read app/org.sql: {}", e))
    }

    /// Combine system and app schemas (for main.db)
    pub fn combine_main_schemas(&self) -> Result<String, String> {
        let system = self.read_system_main()?;
        let app = self.read_app_main()?;
        
        Ok(format!(
            "-- ============================================================\n\
             -- TrailBase System Schema (auto-generated, do not edit directly)\n\
             -- Last synced: {}\n\
             -- See: traildepot/schema/system/main.sql\n\
             -- ============================================================\n\
             {}\n\n\
             -- ============================================================\n\
             -- Application Business Logic Schema (user-editable)\n\
             -- See: traildepot/schema/app/main.sql\n\
             -- ============================================================\n\
             {}",
            Utc::now().to_rfc3339(),
            system,
            app
        ))
    }

    /// Combine system and app schemas (for org_*.db)
    pub fn combine_org_schemas(&self) -> Result<String, String> {
        let system = self.read_system_org()?;
        let app = self.read_app_org()?;
        
        Ok(format!(
            "-- ============================================================\n\
             -- TrailBase Org-Scoped System Schema (auto-generated)\n\
             -- Last synced: {}\n\
             -- See: traildepot/schema/system/org.sql\n\
             -- ============================================================\n\
             {}\n\n\
             -- ============================================================\n\
             -- App Org-Scoped Business Logic (user-editable)\n\
             -- See: traildepot/schema/app/org.sql\n\
             -- ============================================================\n\
             {}",
            Utc::now().to_rfc3339(),
            system,
            app
        ))
    }

    /// Ensure migrations directories exist
    pub fn ensure_migrations_dirs(&self) -> Result<(), String> {
        fs::create_dir_all(&self.migrations_main)
            .map_err(|e| format!("Failed to create migrations/main: {}", e))?;
        
        fs::create_dir_all(&self.migrations_org)
            .map_err(|e| format!("Failed to create migrations/orgs: {}", e))?;

        Ok(())
    }

    /// Generate migration filename with timestamp
    pub fn generate_migration_filename(name: &str) -> String {
        let now = Utc::now();
        format!(
            "U{}__{}.sql",
            now.format("%Y%m%d_%H%M%S"),
            name.replace(" ", "_").to_lowercase()
        )
    }

    /// Create a migration file
    pub fn create_migration(
        &self,
        migration_type: MigrationType,
        name: &str,
        sql: &str,
    ) -> Result<PathBuf, String> {
        self.ensure_migrations_dirs()?;

        let migrations_dir = match migration_type {
            MigrationType::Main => &self.migrations_main,
            MigrationType::Org(_) => &self.migrations_org,
        };

        let filename = Self::generate_migration_filename(name);
        let path = migrations_dir.join(&filename);

        fs::write(&path, sql)
            .map_err(|e| format!("Failed to write migration: {}", e))?;

        Ok(path)
    }

    /// Check if system/ has changes compared to git
    pub fn system_has_changes(&self) -> Result<bool, String> {
        if !self.system_main.exists() || !self.system_org.exists() {
            return Ok(false);
        }

        // Run: git diff --quiet traildepot/schema/system/
        let output = Command::new("git")
            .arg("diff")
            .arg("--quiet")
            .arg("traildepot/schema/system/")
            .current_dir(&self.base_dir)
            .output()
            .map_err(|e| format!("Failed to run git diff: {}", e))?;

        // Exit code 0 = no changes, exit code 1 = has changes
        Ok(!output.status.success())
    }

    /// Get git diff of system/ changes
    pub fn get_system_changes_diff(&self) -> Result<String, String> {
        let output = Command::new("git")
            .arg("diff")
            .arg("traildepot/schema/system/")
            .current_dir(&self.base_dir)
            .output()
            .map_err(|e| format!("Failed to get git diff: {}", e))?;

        String::from_utf8(output.stdout)
            .map_err(|e| format!("Failed to decode diff output: {}", e))
    }

    /// Get list of migrations applied (from filesystem)
    pub fn list_migrations_main(&self) -> Result<Vec<String>, String> {
        if !self.migrations_main.exists() {
            return Ok(Vec::new());
        }

        let mut migrations = Vec::new();
        for entry in fs::read_dir(&self.migrations_main)
            .map_err(|e| format!("Failed to read migrations/main: {}", e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read dir entry: {}", e))?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "sql") {
                migrations.push(
                    path.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .to_string(),
                );
            }
        }

        migrations.sort();
        Ok(migrations)
    }

    /// Print schema structure info
    pub fn print_info(&self) {
        println!("\n╭─ TrailBase Schema Structure ─────────────────────────╮");
        println!("│");
        println!("│ 📁 System Schemas (auto-generated):");
        println!("│   • system/main.sql  - Core TrailBase tables (_user, _org, etc)");
        println!("│   • system/org.sql   - Org-scoped system tables");
        println!("│");
        println!("│ 📝 Application Schemas (user-editable):");
        println!("│   • app/main.sql     - Business logic tables (contracts, properties, etc)");
        println!("│   • app/org.sql      - Org-scoped business tables");
        println!("│");
        println!("│ 🔄 Migrations (auto-generated from app/ changes):");
        println!("│   • migrations/main/ - Schema changes for main.db");
        println!("│   • migrations/orgs/ - Per-org migrations");
        println!("│");
        println!("╰───────────────────────────────────────────────────────╯\n");
    }
}

/// Migration type
#[derive(Debug, Clone)]
pub enum MigrationType {
    Main,
    Org(String), // org_slug
}

/// Result of schema initialization
#[derive(Debug)]
pub struct InitResult {
    pub system_applied: bool,
    pub app_applied: bool,
    pub migrations_created: Vec<String>,
    pub status: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_migration_filename() {
        let filename = SchemaStructure::generate_migration_filename("add user table");
        assert!(filename.starts_with("U"));
        assert!(filename.ends_with(".sql"));
        assert!(filename.contains("add_user_table"));
    }
}
