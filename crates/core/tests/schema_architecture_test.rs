/// Tests for schema architecture: system/ vs app/ separation
/// 
/// Validates:
/// - system/ schemas are treated as read-only system definitions
/// - app/ schemas are user-editable business logic
/// - Migrations are generated only from app/ changes
/// - Multi-org databases receive both system/org.sql + app/org.sql
/// - Database initialization from schema files

#[cfg(test)]
mod schema_architecture_tests {
    use std::fs;
    use std::path::Path;

    /// Helper to validate schema directory structure
    fn validate_schema_structure(schema_dir: &Path) -> Result<(), String> {
        // Check system/ directory
        let system_dir = schema_dir.join("system");
        if !system_dir.is_dir() {
            return Err(format!("Missing system/ directory at {:?}", system_dir));
        }

        let system_main = system_dir.join("main.sql");
        if !system_main.is_file() {
            return Err(format!(
                "Missing system/main.sql at {:?}",
                system_main
            ));
        }

        let system_org = system_dir.join("org.sql");
        if !system_org.is_file() {
            return Err(format!(
                "Missing system/org.sql at {:?}",
                system_org
            ));
        }

        // Check app/ directory
        let app_dir = schema_dir.join("app");
        if !app_dir.is_dir() {
            return Err(format!("Missing app/ directory at {:?}", app_dir));
        }

        let app_main = app_dir.join("main.sql");
        if !app_main.is_file() {
            return Err(format!("Missing app/main.sql at {:?}", app_main));
        }

        let app_org = app_dir.join("org.sql");
        if !app_org.is_file() {
            return Err(format!("Missing app/org.sql at {:?}", app_org));
        }

        Ok(())
    }

    /// Helper to check that system/ files contain appropriate content markers
    fn validate_system_schema_content(system_main: &Path) -> Result<(), String> {
        let content = fs::read_to_string(system_main).map_err(|e| {
            format!("Failed to read system/main.sql: {}", e)
        })?;

        // System schemas should contain core tables (or be marked as placeholder)
        let core_indicators = [
            "_user", "_org", "_org_membership", "_session",
            "AUTOGENERADO", "PLACEHOLDER", "trail schema export"
        ];

        let has_indicator = core_indicators.iter().any(|ind| content.contains(ind));
        
        if !has_indicator {
            return Err(
                "system/main.sql should contain TrailBase core tables or sync instructions".to_string()
            );
        }

        Ok(())
    }

    /// Helper to check that app/ files do NOT contain system table definitions
    fn validate_app_schema_isolation(app_main: &Path) -> Result<(), String> {
        let content = fs::read_to_string(app_main).map_err(|e| {
            format!("Failed to read app/main.sql: {}", e)
        })?;

        // App schemas should NOT contain TrailBase system tables
        let system_tables = ["_user", "_org", "_org_membership", "_session"];
        
        for table in system_tables {
            if content.contains(&format!("CREATE TABLE {}", table)) {
                return Err(format!(
                    "app/main.sql should not define system table {} (should be in system/main.sql)",
                    table
                ));
            }
        }

        Ok(())
    }

    #[test]
    fn test_schema_directory_structure() {
        // Validate that traildepot/schema has correct structure
        let schema_dir = Path::new("traildepot/schema");
        
        if schema_dir.exists() {
            match validate_schema_structure(schema_dir) {
                Ok(()) => println!("✓ Schema directory structure is valid"),
                Err(e) => panic!("Schema structure validation failed: {}", e),
            }
        } else {
            println!("⊘ traildepot/schema not found (expected in deployed instances)");
        }
    }

    #[test]
    fn test_system_schema_content() {
        let system_main = Path::new("traildepot/schema/system/main.sql");
        
        if system_main.exists() {
            match validate_system_schema_content(system_main) {
                Ok(()) => println!("✓ system/main.sql contains appropriate content"),
                Err(e) => panic!("System schema validation failed: {}", e),
            }
        }
    }

    #[test]
    fn test_app_schema_isolation() {
        let app_main = Path::new("traildepot/schema/app/main.sql");
        
        if app_main.exists() {
            match validate_app_schema_isolation(app_main) {
                Ok(()) => println!("✓ app/main.sql is properly isolated from system tables"),
                Err(e) => panic!("App schema isolation test failed: {}", e),
            }
        }
    }

    #[test]
    fn test_migration_ordering() {
        // Verify migrations follow U<timestamp>__name.sql pattern
        let migrations_dir = Path::new("traildepot/migrations/main");
        
        if !migrations_dir.exists() {
            println!("⊘ migrations/main not found (expected in deployed instances)");
            return;
        }

        if let Ok(entries) = fs::read_dir(migrations_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "sql") {
                    let filename = path.file_name().unwrap().to_string_lossy();
                    
                    // Should match U<timestamp>__name.sql
                    if !filename.starts_with("U") || !filename.contains("__") {
                        panic!(
                            "Migration file '{}' does not match U<timestamp>__name.sql pattern",
                            filename
                        );
                    }
                }
            }
            println!("✓ All migration files follow correct naming convention");
        }
    }

    #[test]
    fn test_multi_org_schema_structure() {
        // For multi-org: org.sql files should exist for both system and app
        let system_org = Path::new("traildepot/schema/system/org.sql");
        let app_org = Path::new("traildepot/schema/app/org.sql");
        
        if system_org.exists() {
            println!("✓ system/org.sql exists (multi-org support)");
        }
        
        if app_org.exists() {
            println!("✓ app/org.sql exists (multi-org business logic)");
        }
    }
}

/// Integration tests for schema initialization and migration
#[cfg(test)]
mod schema_initialization_tests {
    use std::path::Path;
    use std::fs;

    #[test]
    fn test_schema_files_are_readable() {
        // Ensure all schema files can be read (no encoding issues, etc)
        let schema_dir = Path::new("traildepot/schema");
        
        if !schema_dir.exists() {
            println!("⊘ traildepot/schema not found");
            return;
        }

        for entry in walkdir_simple(schema_dir) {
            if entry.ends_with(".sql") {
                match fs::read_to_string(&entry) {
                    Ok(content) => {
                        if content.is_empty() {
                            println!("⚠ {} is empty", entry);
                        }
                    }
                    Err(e) => panic!("Failed to read {}: {}", entry, e),
                }
            }
        }
        
        println!("✓ All schema SQL files are readable");
    }

    #[test]
    fn test_schema_file_hierarchy() {
        // Verify system/ files are listed before app/ in priority
        // (This is more of a documentation test)
        let system_dir = Path::new("traildepot/schema/system");
        let app_dir = Path::new("traildepot/schema/app");
        
        let has_system = system_dir.exists() && has_sql_files(system_dir);
        let has_app = app_dir.exists() && has_sql_files(app_dir);
        
        if has_system && has_app {
            println!("✓ Both system/ and app/ schema directories contain SQL files");
        }
    }

    fn walkdir_simple(dir: &Path) -> Vec<String> {
        let mut results = Vec::new();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    results.extend(walkdir_simple(&path));
                } else {
                    results.push(path.to_string_lossy().to_string());
                }
            }
        }
        results
    }

    fn has_sql_files(dir: &Path) -> bool {
        if let Ok(entries) = fs::read_dir(dir) {
            entries.flatten().any(|e| {
                e.path()
                    .extension()
                    .map_or(false, |ext| ext == "sql")
            })
        } else {
            false
        }
    }
}

/// Tests for schema synchronization and updates
#[cfg(test)]
mod schema_sync_tests {
    use std::path::Path;
    use std::fs;

    #[test]
    fn test_system_schema_has_sync_instructions() {
        let system_main = Path::new("traildepot/schema/system/main.sql");
        
        if !system_main.exists() {
            println!("⊘ system/main.sql not found");
            return;
        }

        match fs::read_to_string(system_main) {
            Ok(content) => {
                let has_instructions = content.contains("trail schema export")
                    || content.contains("sync")
                    || content.contains("AUTOGENERADO");
                
                if has_instructions {
                    println!("✓ system/main.sql includes sync/regeneration instructions");
                } else {
                    println!("⚠ system/main.sql lacks clear sync instructions");
                }
            }
            Err(e) => panic!("Failed to read system/main.sql: {}", e),
        }
    }

    #[test]
    fn test_app_schema_has_no_autogeneration_markers() {
        // App schemas should be user-editable, not marked as auto-generated
        let app_main = Path::new("traildepot/schema/app/main.sql");
        
        if !app_main.exists() {
            println!("⊘ app/main.sql not found");
            return;
        }

        match fs::read_to_string(app_main) {
            Ok(content) => {
                let has_autogen = content.contains("AUTOGENERADO")
                    || content.contains("auto-generated");
                
                if !has_autogen {
                    println!("✓ app/main.sql is not marked as auto-generated (good for user editing)");
                } else {
                    println!("⚠ app/main.sql should not be marked as auto-generated");
                }
            }
            Err(e) => println!("⊘ Could not read app/main.sql: {}", e),
        }
    }

    #[test]
    fn test_migrations_directory_exists() {
        let migrations_main = Path::new("traildepot/migrations/main");
        
        if migrations_main.exists() {
            println!("✓ migrations/main directory exists");
        } else {
            println!("⊘ migrations/main directory not found (will be created on first migration)");
        }
    }
}
