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
        db: Option<String>,
        _force: bool,
    ) -> Result<(), BoxError> {
        let schema = SchemaStructure::detect(&self.base_dir)?;
        let data_dir = self.base_dir.join("data");
        let main_db_path = data_dir.join("main.db");
        let logs_db_path = data_dir.join("logs.db");
        let session_db_path = data_dir.join("session.db");
        let queue_db_path = data_dir.join("queue.db");

        if !main_db_path.is_file() {
            return Err(format!("Main database not found: {}", main_db_path.display()).into());
        }

        #[derive(Clone, Copy)]
        enum SyncTarget {
            Main,
            Org,
            Session,
            Logs,
            Queue,
        }

        let selected = match db.as_deref() {
            None | Some("") | Some("all") => vec![
                SyncTarget::Main,
                SyncTarget::Org,
                SyncTarget::Session,
                SyncTarget::Logs,
                SyncTarget::Queue,
            ],
            Some("main") => vec![SyncTarget::Main],
            Some("org") => vec![SyncTarget::Org],
            Some("session") => vec![SyncTarget::Session],
            Some("logs") => vec![SyncTarget::Logs],
            Some("queue") => vec![SyncTarget::Queue],
            Some(other) => {
                return Err(format!(
                    "Unsupported --db value '{other}'. Use one of: main, org, session, logs, queue, all"
                )
                .into())
            }
        };

        for target in &selected {
            let output_path = match target {
                SyncTarget::Main => &schema.system_main,
                SyncTarget::Org => &schema.system_org,
                SyncTarget::Session => &schema.system_session,
                SyncTarget::Logs => &schema.system_logs,
                SyncTarget::Queue => &schema.system_queue,
            };

            if !_force && output_path.exists() {
                let existing = fs::read_to_string(output_path).unwrap_or_default();
                if !existing.trim().is_empty() {
                    return Err(format!(
                        "{} already exists; rerun with --force to overwrite it",
                        output_path.display()
                    )
                    .into());
                }
            }
        }

        #[derive(serde::Deserialize)]
        struct SchemaRow {
            name: String,
            sql: Option<String>,
        }

        fn collect_schema_statements(
            conn: &rusqlite::Connection,
            include_only_internal_objects: bool,
        ) -> Result<Vec<String>, BoxError> {
            let mut statements: Vec<String> = vec![];

            let filter_clause = if include_only_internal_objects {
                "name LIKE '\\_%' ESCAPE '\\'"
            } else {
                "name NOT LIKE 'sqlite_%'"
            };

            let mut collect = |query: String| -> Result<(), BoxError> {
                let mut stmt = conn.prepare(&query)?;
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

            for object_type in ["table", "index", "trigger", "view"] {
                collect(format!(
                    "SELECT name, sql FROM sqlite_schema WHERE type = '{object_type}' AND sql IS NOT NULL AND {filter_clause} ORDER BY name"
                ))?;
            }

            Ok(statements)
        }

        fn format_system_schema_output(db_label: &str, statements: &[String]) -> String {
            let mut output = format!(
                "-- ⚠️ AUTOGENERADO - NO MODIFICAR MANUALMENTE\n\
                 -- ============================================================\n\
                 -- TrailBase system schema\n\
                 -- Source: generated from the live {db_label}.db\n\
                 -- ============================================================\n\n"
            );

            if statements.is_empty() {
                output.push_str("-- [SIN OBJETOS DE SISTEMA PARA EXPORTAR]\n");
                return output;
            }

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
            output
        }

        fn placeholder_output(db_label: &str, reason: &str) -> String {
            format!(
                "-- ⚠️ AUTOGENERADO - NO MODIFICAR MANUALMENTE\n\
                 -- ============================================================\n\
                 -- TrailBase system schema\n\
                 -- Source: generated from the live {db_label}.db\n\
                 -- ============================================================\n\n\
                 -- [PLACEHOLDER] {reason}\n"
            )
        }

        if let Some(parent) = schema.system_main.parent() {
            fs::create_dir_all(parent)?;
        }

        for target in selected {
            match target {
                SyncTarget::Main => {
                    let conn = rusqlite::Connection::open(&main_db_path)?;
                    let statements = collect_schema_statements(&conn, true)?;
                    fs::write(
                        &schema.system_main,
                        format_system_schema_output("main", &statements),
                    )?;
                    println!("✅ {} actualizado", schema.system_main.display());
                }
                SyncTarget::Org => {
                    let mut org_dbs: Vec<PathBuf> = fs::read_dir(&data_dir)?
                        .filter_map(|entry| entry.ok().map(|e| e.path()))
                        .filter(|path| {
                            path.is_file()
                                && path
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .map(|n| n.starts_with("org_") && n.ends_with(".db"))
                                    .unwrap_or(false)
                        })
                        .collect();
                    org_dbs.sort();

                    let output = if let Some(org_db_path) = org_dbs.first() {
                        let conn = rusqlite::Connection::open(org_db_path)?;
                        let statements = collect_schema_statements(&conn, true)?;
                        format_system_schema_output("org", &statements)
                    } else {
                        placeholder_output(
                            "org",
                            "No org_*.db file found yet. Create or open an org DB and rerun sync.",
                        )
                    };

                    fs::write(&schema.system_org, output)?;
                    println!("✅ {} actualizado", schema.system_org.display());
                }
                SyncTarget::Session => {
                    let output = if session_db_path.is_file() {
                        let conn = rusqlite::Connection::open(&session_db_path)?;
                        let statements = collect_schema_statements(&conn, false)?;
                        format_system_schema_output("session", &statements)
                    } else {
                        placeholder_output(
                            "session",
                            "session.db not found yet. Run TrailBase once and rerun sync.",
                        )
                    };
                    fs::write(&schema.system_session, output)?;
                    println!("✅ {} actualizado", schema.system_session.display());
                }
                SyncTarget::Logs => {
                    let output = if logs_db_path.is_file() {
                        let conn = rusqlite::Connection::open(&logs_db_path)?;
                        let statements = collect_schema_statements(&conn, false)?;
                        format_system_schema_output("logs", &statements)
                    } else {
                        placeholder_output(
                            "logs",
                            "logs.db not found yet. Run TrailBase once and rerun sync.",
                        )
                    };
                    fs::write(&schema.system_logs, output)?;
                    println!("✅ {} actualizado", schema.system_logs.display());
                }
                SyncTarget::Queue => {
                    let output = if queue_db_path.is_file() {
                        let conn = rusqlite::Connection::open(&queue_db_path)?;
                        let statements = collect_schema_statements(&conn, false)?;
                        format_system_schema_output("queue", &statements)
                    } else {
                        placeholder_output(
                            "queue",
                            "queue.db not found yet. Queue runtime is not initialized in this environment.",
                        )
                    };
                    fs::write(&schema.system_queue, output)?;
                    println!("✅ {} actualizado", schema.system_queue.display());
                }
            }
        }

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
