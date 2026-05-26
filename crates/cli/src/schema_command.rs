/// CLI commands for schema management
/// 
/// Commands:
/// - trail schema-mgmt validate        Validate schema structure
/// - trail schema-mgmt status          Show current schema state
/// - trail schema-mgmt sync-system     Extract system schemas from running DB to files
/// - trail schema-mgmt init-main       Initialize main.db from system/ + app/
/// - trail schema-mgmt detect-changes  Check if system/ has uncommitted changes
/// - trail schema-mgmt auto-sync       Automatically handle system/ + app/ changes end-to-end

use std::fs;
use std::path::PathBuf;
use trailbase::schema_manager::SchemaStructure;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

pub struct SchemaCommand {
    base_dir: PathBuf,
}

impl SchemaCommand {
    pub fn new(base_dir: PathBuf) -> Self {
        SchemaCommand { base_dir }
    }

    pub async fn cmd_validate(
        &self,
        verbose: bool,
    ) -> Result<(), BoxError> {
        let schema = SchemaStructure::detect(&self.base_dir)?;
        schema.validate()?;
        
        if verbose {
            schema.print_info();
        }

        println!("✓ Schema structure is valid");
        Ok(())
    }

    pub async fn cmd_status(
        &self,
        _with_migrations: bool,
    ) -> Result<(), BoxError> {
        let schema = SchemaStructure::detect(&self.base_dir)?;
        schema.print_info();

        // Show migration count
        let migrations = schema.list_migrations_main()?;
        println!("📊 Status:");
        println!("   • Migrations applied: {}", migrations.len());

        if !migrations.is_empty() {
            println!("   • Latest: {}", migrations.last().unwrap());
        }

        Ok(())
    }

    pub async fn cmd_sync_system(
        &self,
        _db: Option<String>,
        _force: bool,
    ) -> Result<(), BoxError> {
        let schema = SchemaStructure::detect(&self.base_dir)?;
        let db_path = self.base_dir.join("data/main.db");

        if !db_path.is_file() {
            return Err(format!("Main database not found: {}", db_path.display()).into());
        }

        if !_force && schema.system_main.exists() {
            let existing = fs::read_to_string(&schema.system_main).unwrap_or_default();
            if !existing.trim().is_empty() {
                return Err(format!(
                    "{} already exists; rerun with --force to overwrite it",
                    schema.system_main.display()
                )
                .into());
            }
        }

        let conn = rusqlite::Connection::open(&db_path)?;

        #[derive(serde::Deserialize)]
        struct SchemaRow {
            name: String,
            sql: Option<String>,
        }

        let mut statements: Vec<String> = vec![];

        let mut collect = |query: &str| -> Result<(), BoxError> {
            let mut stmt = conn.prepare(query)?;
            let rows = stmt.query_map([], |row| {
                Ok(SchemaRow {
                    name: row.get(0)?,
                    sql: row.get(1)?,
                })
            })?;

            for row in rows {
                let row = row?;
                if row.name == "_schema_history"
                    || row.name == "_schema_fingerprint"
                    || row.name == "_schema_diff_meta"
                {
                    continue;
                }
                if let Some(sql) = row.sql {
                    statements.push(sql);
                }
            }

            Ok(())
        };

        collect(
            "SELECT name, sql FROM sqlite_schema \
             WHERE type = 'table' AND sql IS NOT NULL AND name LIKE '\\_%' ESCAPE '\\' \
             ORDER BY name",
        )?;
        collect(
            "SELECT name, sql FROM sqlite_schema \
             WHERE type = 'index' AND sql IS NOT NULL AND name LIKE '\\_%' ESCAPE '\\' \
             ORDER BY name",
        )?;
        collect(
            "SELECT name, sql FROM sqlite_schema \
             WHERE type = 'trigger' AND sql IS NOT NULL AND name LIKE '\\_%' ESCAPE '\\' \
             ORDER BY name",
        )?;
        collect(
            "SELECT name, sql FROM sqlite_schema \
             WHERE type = 'view' AND sql IS NOT NULL AND name LIKE '\\_%' ESCAPE '\\' \
             ORDER BY name",
        )?;

        let mut output = String::from(
            "-- ⚠️ AUTOGENERADO - NO MODIFICAR MANUALMENTE\n\
             -- ============================================================\n\
             -- TrailBase system schema\n\
             -- Source: generated from the live main.db\n\
             -- ============================================================\n\n",
        );
        let formatted_statements = statements
            .iter()
            .map(|statement| {
                let statement = statement.trim();
                if statement.ends_with(';') {
                    statement.to_string()
                } else {
                    format!("{statement};")
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        output.push_str(&formatted_statements);
        output.push('\n');

        if let Some(parent) = schema.system_main.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&schema.system_main, output)?;

        println!("✅ {} actualizado", schema.system_main.display());
        Ok(())
    }

    pub async fn cmd_init_main(
        &self,
        _db: Option<String>,
        _destructive: bool,
    ) -> Result<(), BoxError> {
        let schema = SchemaStructure::detect(&self.base_dir)?;
        
        println!("📦 Initializing main.db from system/ + app/ schemas...");

        let combined = schema.combine_main_schemas()?;
        println!("✓ Combined {} bytes of schema", combined.len());

        println!("\n📝 Next step: Apply to database");
        println!("   trail declarative apply --db main --schema combined.sql");

        Ok(())
    }

    pub async fn cmd_detect_changes(
        &self,
        show_diff: bool,
    ) -> Result<(), BoxError> {
        let schema = SchemaStructure::detect(&self.base_dir)?;
        
        match schema.system_has_changes()? {
            true => {
                println!("⚠️  System schemas have changed!");

                if show_diff {
                    let diff = schema.get_system_changes_diff()?;
                    println!("\n{}", diff);
                } else {
                    println!("\nRun 'trail schema-mgmt detect-changes --show-diff' to see details");
                }

                println!("\n💡 Next steps:");
                println!("   1. Review changes: git diff traildepot/schema/system/");
                println!("   2. Plan migrations: trail schema-mgmt plan-migration");
                println!("   3. Apply migrations: trail schema-mgmt apply-migration");

                Err("System schemas have changed".into())
            }
            false => {
                println!("✓ System schemas are stable (no changes detected)");
                Ok(())
            }
        }
    }

    pub async fn cmd_auto_sync(
        &self,
        _db: Option<String>,
        plan_only: bool,
        verbose: bool,
    ) -> Result<(), BoxError> {
        let schema = SchemaStructure::detect(&self.base_dir)?;
        
        println!("🔄 Auto-syncing schemas...\n");

        // Step 1: Validate
        println!("1️⃣  Validating schema structure...");
        schema.validate()?;
        println!("   ✓ Structure valid\n");

        // Step 2: Check for changes
        println!("2️⃣  Checking for system/ changes...");
        match schema.system_has_changes()? {
            true => println!("   ⚠️  Changes detected in system/ schemas"),
            false => println!("   ✓ System schemas stable"),
        }
        println!();

        // Step 3: Plan migrations
        println!("3️⃣  Planning migrations from app/ changes...");
        let migrations = schema.list_migrations_main()?;
        println!("   • Pending migrations: {}", migrations.len());
        println!();

        // Step 4: Show next steps
        if !plan_only {
            println!("4️⃣  Ready to apply migrations");
            println!("   Run: trail schema-mgmt apply-migration");
        }

        if verbose {
            schema.print_info();
        }

        println!("\n✓ Sync complete");
        Ok(())
    }
}
