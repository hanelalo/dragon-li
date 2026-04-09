mod chat_provider;
mod commands;
mod config_guardrails;
mod memory_pipeline;
mod runtime;
mod sqlite_store;
mod utils;

use runtime::AppState;
use tauri::Manager;
use tracing::info;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

fn main() {
    // Set up file logger before Tauri builder to avoid panicking inside the setup hook
    if let Some(home) = dirs::home_dir() {
        let logs_dir = home.join(".dragon-li").join("logs");
        let _ = std::fs::create_dir_all(&logs_dir);
        
        // Use a standard file appender that won't panic if it fails
        if let Ok(log_file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(logs_dir.join("dragon-li.log"))
        {
            let (non_blocking, _guard) = tracing_appender::non_blocking(log_file);
            Box::leak(Box::new(_guard));

            tracing_subscriber::registry()
                .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
                .with(fmt::layer().with_writer(non_blocking))
                .with(fmt::layer().with_writer(std::io::stdout)) // also log to stdout
                .init();
                
            info!("Logger initialized in ~/.dragon-li/logs/");
        } else {
            // Fallback to stdout only
            tracing_subscriber::registry()
                .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
                .with(fmt::layer().with_writer(std::io::stdout))
                .init();
            tracing::warn!("Failed to create log file, falling back to stdout");
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = match AppState::bootstrap() {
                Ok(state) => state,
                Err(err) => {
                    tracing::error!("failed to bootstrap runtime: {err}");
                    std::process::exit(1);
                }
            };
            
            // Initialize database schema explicitly during setup
            if let Err(err) = state.sqlite_store.init_schema() {
                tracing::error!("Failed to initialize database schema: {err}");
            } else {
                tracing::info!("Database schema initialized successfully during setup");
            }

            if let Err(err) = state.agent_manager.lock().unwrap().start(app.handle()) {
                tracing::error!("failed to start agent: {err}");
            }

            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::system::ping,
            commands::system::runtime_info,
            commands::agent::start_agent,
            commands::agent::stop_agent,
            commands::agent::agent_status,
            commands::agent::agent_health_check,
            commands::config::config_get,
            commands::config::config_save,
            commands::config::config_check_external_change,
            commands::config::config_apply_external_change,
            commands::guardrail::guardrail_validate_path,
            commands::guardrail::guardrail_validate_capability,
            commands::guardrail::guardrail_validate_network,
            commands::session::db_init,
            commands::session::session_create,
            commands::session::session_list,
            commands::session::session_update_title,
            commands::session::message_create,
            commands::session::message_list,
            commands::session::request_log_create,
            commands::session::request_log_list_by_request_id,
            commands::session::session_soft_delete,
            commands::session::session_restore,
            commands::chat::chat_summarize_title,
            commands::chat::chat_send,
            commands::mcp::mcp_connector_create,
            commands::mcp::mcp_connector_list,
            commands::mcp::mcp_connector_update,
            commands::mcp::mcp_connector_delete,
            commands::mcp::mcp_connector_test,
            commands::mcp::mcp_get_status,
            commands::skill::skill_list,
            commands::skill::skill_toggle,
            commands::skill::skill_rescan,
            commands::memory::memory_extract_candidates,
            commands::memory::memory_count_pending,
            commands::memory::memory_list_candidates,
            commands::memory::memory_review_candidate,
            commands::memory::memory_soft_delete,
            commands::memory::memory_restore,
            commands::memory::memory_read,
            commands::memory::memory_search,
            commands::memory::memory_list_long_term
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| match event {
            tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit => {
                tracing::info!("App exit requested, stopping agent...");
                if let Some(state) = app_handle.try_state::<AppState>() {
                    if let Ok(mut manager) = state.agent_manager.lock() {
                        let _ = manager.stop();
                    }
                }
            }
            _ => {}
        });
}
