use super::command::{AdminCommand, GcArgs, ResetArgs};
use super::context::build_admin_context;
use super::io::{print_deleted, print_plan};
use super::paths::{data_dir_from_db_path, remove_paths, resolve_config_path};
use palette_domain::workflow::WorkflowId;
use palette_usecase::{AdminGcOptions, AdminMaintenanceError};

pub fn run(command: AdminCommand) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        AdminCommand::Reset(args) => run_reset(args),
        AdminCommand::Gc(args) => run_gc(args),
    }
}

fn run_reset(args: ResetArgs) -> Result<(), Box<dyn std::error::Error>> {
    if !args.dry_run && !args.yes {
        return Err("refusing destructive operation: pass --yes (or use --dry-run)".into());
    }

    let config_path = resolve_config_path(args.config.as_deref())?;
    let context = build_admin_context(&config_path)?;
    let data_dir = data_dir_from_db_path(&context.config.db_path);

    let plan = context
        .interactor
        .admin_plan_reset(&data_dir)
        .map_err(to_box_error)?;
    print_plan("reset", &plan);

    if args.dry_run {
        println!("dry-run: no changes applied");
        return Ok(());
    }

    let deleted = context
        .interactor
        .admin_execute_cleanup(&plan.workflow_ids)
        .map_err(to_box_error)?;
    let removed_files = remove_paths(&plan.file_paths);
    print_deleted(&deleted, removed_files);
    Ok(())
}

fn run_gc(args: GcArgs) -> Result<(), Box<dyn std::error::Error>> {
    if !args.dry_run && !args.yes {
        return Err("refusing destructive operation: pass --yes (or use --dry-run)".into());
    }

    let config_path = resolve_config_path(args.config.as_deref())?;
    let context = build_admin_context(&config_path)?;
    let data_dir = data_dir_from_db_path(&context.config.db_path);

    let workflow_ids = parse_workflow_ids(&args.workflow_ids)?;
    let options = AdminGcOptions {
        workflow_ids,
        include_active: args.include_active,
        older_than_hours: args.older_than_hours,
    };
    let plan = context
        .interactor
        .admin_plan_gc(&data_dir, &options)
        .map_err(to_box_error)?;
    if plan.workflow_ids.is_empty() {
        println!("gc: no matching workflows");
        return Ok(());
    }
    print_plan("gc", &plan);

    if args.dry_run {
        println!("dry-run: no changes applied");
        return Ok(());
    }

    let deleted = context
        .interactor
        .admin_execute_cleanup(&plan.workflow_ids)
        .map_err(to_box_error)?;
    let removed_files = remove_paths(&plan.file_paths);
    print_deleted(&deleted, removed_files);
    Ok(())
}

fn parse_workflow_ids(ids: &[String]) -> Result<Vec<WorkflowId>, Box<dyn std::error::Error>> {
    ids.iter()
        .map(|id| {
            WorkflowId::parse(id.clone())
                .map_err(|e| format!("invalid workflow-id '{id}': {e:?}").into())
        })
        .collect()
}

fn to_box_error(e: AdminMaintenanceError) -> Box<dyn std::error::Error> {
    Box::new(e)
}
