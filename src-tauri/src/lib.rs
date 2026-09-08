use kairos_application::{
    models::{Action, Command},
    Application,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;

struct Runtime {
    application: Arc<Mutex<Application>>,
    kill: Arc<AtomicBool>,
}
#[tauri::command]
async fn get_runtime_status(state: tauri::State<'_, Runtime>) -> Result<serde_json::Value, String> {
    serde_json::to_value(state.application.lock().await.snapshot())
        .map_err(|_| "Cannot encode snapshot".into())
}
#[tauri::command]
async fn runtime_command(
    command: Command,
    state: tauri::State<'_, Runtime>,
) -> Result<serde_json::Value, String> {
    if matches!(command.action, Action::Kill) {
        state.kill.store(true, Ordering::SeqCst);
    }
    let snapshot = state.application.lock().await.command(command).await?;
    serde_json::to_value(snapshot).map_err(|_| "Cannot encode snapshot".into())
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let application =
                Application::open(&app.path().app_data_dir()?).map_err(std::io::Error::other)?;
            let kill = application.kill.clone();
            let application = Arc::new(Mutex::new(application));
            app.manage(Runtime {
                application: application.clone(),
                kill,
            });
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    interval.tick().await;
                    let mut runtime = application.lock().await;
                    if let Err(error) = runtime.tick().await {
                        eprintln!("Supervisor: {error}");
                    }
                    if let Ok(snapshot) = serde_json::to_value(runtime.snapshot()) {
                        let _ = handle.emit("runtime.snapshot", snapshot);
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_runtime_status,
            runtime_command
        ])
        .run(tauri::generate_context!())
        .expect("Kairos Agent failed to start");
}
