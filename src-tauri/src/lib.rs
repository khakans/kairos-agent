use kairos_application::{
    models::{Action, Command, Snapshot},
    Application,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;

struct Runtime {
    updates: tokio::sync::watch::Receiver<Arc<Snapshot>>,
    application: Arc<Mutex<Application>>,
    cycle_stop: Arc<std::sync::atomic::AtomicBool>,
    kill: Arc<AtomicBool>,
}
#[tauri::command]
async fn get_runtime_status(state: tauri::State<'_, Runtime>) -> Result<serde_json::Value, String> {
    serde_json::to_value(&**state.updates.borrow()).map_err(|_| "Cannot encode snapshot".into())
}
#[tauri::command]
async fn runtime_command(
    command: Command,
    state: tauri::State<'_, Runtime>,
) -> Result<serde_json::Value, String> {
    if matches!(
        command.action,
        Action::StopCycles | Action::Pause | Action::Kill | Action::StopSession
    ) {
        state.cycle_stop.store(true, Ordering::SeqCst);
    }
    if matches!(command.action, Action::Kill) {
        state.kill.store(true, Ordering::SeqCst);
    }
    let application = state.application.clone();
    let snapshot = tokio::spawn(async move { application.lock().await.command(command).await })
        .await
        .map_err(|_| "Runtime command task stopped")??;
    serde_json::to_value(snapshot).map_err(|_| "Cannot encode snapshot".into())
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let mut application =
                Application::open(&app.path().app_data_dir()?).map_err(std::io::Error::other)?;
            if let Some(worker) = application.discovery_worker() {
                tauri::async_runtime::spawn(worker.run());
            }
            let updates = application.subscribe();
            let mut events = updates.clone();
            let kill = application.kill.clone();
            let cycle_stop = application.cycle_stop.clone();
            let application = Arc::new(Mutex::new(application));
            app.manage(Runtime {
                updates,
                cycle_stop,
                application: application.clone(),
                kill,
            });
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while events.changed().await.is_ok() {
                    let snapshot = events.borrow_and_update().clone();
                    let _ = handle.emit("runtime.snapshot", &*snapshot);
                }
            });
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    interval.tick().await;
                    let mut runtime = application.lock().await;
                    if let Err(error) = runtime.scheduler_tick().await {
                        eprintln!("Supervisor: {error}");
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
