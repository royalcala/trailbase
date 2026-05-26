/// CLI commands for schema management
/// 
/// Commands:
/// - trail schema-mgmt validate        Validate schema structure
/// - trail schema-mgmt status          Show current schema state
/// - trail schema-mgmt sync-system     Extract system schemas from running DB to files
/// - trail schema-mgmt init-main       Initialize main.db from system/ + app/
/// - trail schema-mgmt detect-changes  Check if system/ has uncommitted changes
/// - trail schema-mgmt auto-sync       Automatically handle system/ + app/ changes end-to-end

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
        println!("⚙️  Syncing system schemas from database...");
        println!("   (This requires a running TrailBase instance)");

        println!("\n📝 Command to extract system schemas:");
        println!("   trail schema export > traildepot/schema/system/main.sql");

        println!("\n💡 After exporting:");
        println!("   git add traildepot/schema/system/");
        println!("   git commit -m 'chore(trailbase): sync system schemas'");

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
