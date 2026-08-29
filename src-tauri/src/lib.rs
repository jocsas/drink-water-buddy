// Hydrate Buddy — Tauri port of the Electron app.
// Equivalent of main.js: config persistence, scheduler, tray, windows, IPC.

use parking_lot::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{Local, Timelike};
use serde::{Deserialize, Serialize};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    image::Image, menu::CheckMenuItem, menu::IsMenuItem, menu::Menu, menu::MenuEvent,
    menu::MenuItem, menu::PredefinedMenuItem, menu::Submenu, AppHandle, Emitter, Manager,
    PhysicalPosition, State, WebviewUrl, WebviewWindowBuilder, Wry,
};
use tauri_plugin_log::{Target, TargetKind};

// ---- Configuration -------------------------------------------------------
const ACTIVE_START_HOUR: u32 = 10;
const ACTIVE_END_HOUR: u32 = 23;
const DEFAULT_INTERVAL_MIN: i64 = 45;
const DEFAULT_SNOOZE_MIN: i64 = 10;
const GREETING_DELAY_MS: i64 = 6000;
const TICK_MS: u64 = 30000;
const FORCED_DRINK_SNOOZE_LIMIT: u8 = 3;

const WIN_WIDTH: i32 = 360;
const WIN_HEIGHT: i32 = 430;
const EDGE_MARGIN: i32 = 8;
const INTERVAL_OPTIONS: [i64; 6] = [15, 30, 45, 60, 90, 120];
const SNOOZE_OPTIONS: [i64; 5] = [5, 10, 15, 20, 30];
const THEME_IDS: [&str; 4] = ["default", "baby-yoda", "darth-vader", "wizard"];
const THEME_LABELS: [(&str, &str); 4] = [
    ("default", "Default doll"),
    ("baby-yoda", "Baby Yoda"),
    ("darth-vader", "Darth Vader"),
    ("wizard", "Wizard"),
];
// --------------------------------------------------------------------------

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn theme_label(theme_id: &str) -> String {
    THEME_LABELS
        .iter()
        .find(|(id, _)| *id == theme_id)
        .map(|(_, label)| *label)
        .unwrap_or("Default doll")
        .to_string()
}

fn resolve_theme(raw: &str) -> String {
    let alias = match raw {
        "starwars" => "baby-yoda",
        other => other,
    };
    if THEME_IDS.contains(&alias) {
        alias.to_string()
    } else {
        "default".to_string()
    }
}

// ---- Settings ------------------------------------------------------------
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct Settings {
    name: String,
    interval_min: i64,
    snooze_min: i64,
    theme_id: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            name: String::new(),
            interval_min: DEFAULT_INTERVAL_MIN,
            snooze_min: DEFAULT_SNOOZE_MIN,
            theme_id: "default".to_string(),
        }
    }
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
struct SettingsPatch {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    interval_min: Option<i64>,
    #[serde(default)]
    snooze_min: Option<i64>,
    #[serde(default)]
    theme_id: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct ReminderPayload {
    name: String,
    interval_min: i64,
    snooze_min: i64,
    theme_id: String,
    snooze_count: u8,
    forced_drink: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct SnoozeOutcome {
    snooze_count: u8,
    forced_drink: bool,
    snooze_min: i64,
}

fn should_force_drink(snooze_count: u8) -> bool {
    snooze_count >= FORCED_DRINK_SNOOZE_LIMIT
}

fn next_snooze_count(current: u8) -> u8 {
    current.saturating_add(1).min(FORCED_DRINK_SNOOZE_LIMIT)
}

fn reminder_payload(settings: Settings, snooze_count: u8) -> ReminderPayload {
    ReminderPayload {
        name: settings.name,
        interval_min: settings.interval_min,
        snooze_min: settings.snooze_min,
        theme_id: settings.theme_id,
        snooze_count,
        forced_drink: should_force_drink(snooze_count),
    }
}

fn patch_changes_interval(settings: &Settings, patch: &SettingsPatch) -> bool {
    patch
        .interval_min
        .map(|value| value.clamp(1, 240) != settings.interval_min)
        .unwrap_or(false)
}

fn config_file(app: &AppHandle) -> Option<std::path::PathBuf> {
    let dir = app.path().app_config_dir().ok()?;
    Some(dir.join("hydrate-buddy").join("config.json"))
}

fn load_config(app: &AppHandle) -> Settings {
    let path = match config_file(app) {
        Some(p) => p,
        None => return Settings::default(),
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            let patch: SettingsPatch = serde_json::from_str(&text).unwrap_or_default();
            Settings {
                name: patch.name.unwrap_or_default(),
                interval_min: patch
                    .interval_min
                    .map(|v| v.clamp(1, 240))
                    .unwrap_or(DEFAULT_INTERVAL_MIN),
                snooze_min: patch
                    .snooze_min
                    .map(|v| v.clamp(1, 120))
                    .unwrap_or(DEFAULT_SNOOZE_MIN),
                theme_id: patch
                    .theme_id
                    .as_deref()
                    .map(resolve_theme)
                    .unwrap_or_else(|| "default".to_string()),
            }
        }
        Err(_) => Settings::default(),
    }
}

fn save_config(app: &AppHandle, settings: &Settings) {
    let Some(path) = config_file(app) else {
        return;
    };
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            log::warn!("failed to create config dir {parent:?}: {e}");
        }
    }
    let json = serde_json::json!({
        "name": settings.name,
        "intervalMin": settings.interval_min,
        "snoozeMin": settings.snooze_min,
        "themeId": settings.theme_id,
    });
    let Ok(text) = serde_json::to_string_pretty(&json) else {
        return;
    };
    if let Err(e) = std::fs::write(&path, text) {
        log::warn!("failed to write config {path:?}: {e}");
    }
}

// ---- Shared app state ----------------------------------------------------
struct AppState {
    settings: Mutex<Settings>,
    paused: Mutex<bool>,
    next_reminder_at: Mutex<i64>,
    consecutive_snoozes: Mutex<u8>,
}

fn reminder_delay_ms(s: &Settings) -> i64 {
    s.interval_min * 60_000
}
fn snooze_delay_ms(s: &Settings) -> i64 {
    s.snooze_min * 60_000
}

fn is_within_active_hours() -> bool {
    if cfg!(debug_assertions) {
        return true;
    }
    let hour = Local::now().hour();
    (ACTIVE_START_HOUR..ACTIVE_END_HOUR).contains(&hour)
}

// ---- Window helpers ------------------------------------------------------
fn position_window(app: &AppHandle) {
    let Some(win) = app.get_webview_window("reminder") else {
        return;
    };
    if let Ok(Some(mon)) = app.primary_monitor() {
        let area = mon.work_area();
        // WIN_WIDTH/WIN_HEIGHT are logical (CSS) pixels, matching how
        // inner_size is interpreted by the builder. Work area is physical, so
        // scale the window size by the monitor's factor before anchoring.
        let scale = mon.scale_factor();
        let win_w_phys = WIN_WIDTH as f64 * scale;
        let win_h_phys = WIN_HEIGHT as f64 * scale;
        let x = (area.position.x + area.size.width as i32) as f64 - win_w_phys - EDGE_MARGIN as f64;
        let y =
            (area.position.y + area.size.height as i32) as f64 - win_h_phys - EDGE_MARGIN as f64;
        let _ = win.set_position(PhysicalPosition::new(x as i32, y as i32));
    }
}

fn create_reminder_window(app: &AppHandle) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, "reminder", WebviewUrl::App("index.html".into()))
        .inner_size(WIN_WIDTH as f64, WIN_HEIGHT as f64)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .focused(false)
        .build()?;
    Ok(())
}

fn open_settings(app: &AppHandle) {
    open_settings_with_focus(app, None);
}

fn settings_url(focus: Option<&str>) -> &'static str {
    match focus {
        Some("interval") => "settings.html?focus=interval",
        Some("snooze") => "settings.html?focus=snooze",
        Some("theme") => "settings.html?focus=theme",
        _ => "settings.html?focus=name",
    }
}

fn open_settings_with_focus(app: &AppHandle, focus: Option<&str>) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
        if let Some(target) = focus {
            let _ = app.emit_to("settings", "settings:focus", target);
        }
        return;
    }
    if let Err(e) =
        WebviewWindowBuilder::new(app, "settings", WebviewUrl::App(settings_url(focus).into()))
            .inner_size(430.0, 470.0)
            .title("Hydrate Buddy Settings")
            .resizable(false)
            .maximizable(false)
            .minimizable(false)
            .skip_taskbar(true)
            .build()
    {
        log::error!("failed to open settings window: {e}");
    }
}

// ---- Tray ----------------------------------------------------------------
fn tray_icon_for(theme_id: &str) -> Image<'static> {
    let bytes: &[u8] = match theme_id {
        "baby-yoda" => include_bytes!("../icons/tray/baby-yoda.png"),
        "darth-vader" => include_bytes!("../icons/tray/darth-vader.png"),
        "wizard" => include_bytes!("../icons/tray/wizard.png"),
        _ => include_bytes!("../icons/tray/default.png"),
    };
    let decoded = image::load_from_memory(bytes)
        .expect("decode tray icon")
        .to_rgba8();
    let (width, height) = decoded.dimensions();
    Image::new_owned(decoded.into_raw(), width, height)
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().clone();
    let paused = *state.paused.lock();

    let drink = MenuItem::with_id(app, "drink", "Drink now 💧", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;

    // Interval submenu
    let interval_items: Vec<CheckMenuItem<Wry>> = INTERVAL_OPTIONS
        .iter()
        .map(|minutes| {
            CheckMenuItem::with_id(
                app,
                format!("int_{minutes}"),
                format!("{minutes} min"),
                true,
                settings.interval_min == *minutes,
                None::<&str>,
            )
        })
        .collect::<tauri::Result<_>>()?;
    let int_sep = PredefinedMenuItem::separator(app)?;
    let int_custom = MenuItem::with_id(app, "custom_int", "Custom...", true, None::<&str>)?;
    let mut interval_refs: Vec<&dyn IsMenuItem<Wry>> = interval_items
        .iter()
        .map(|i| i as &dyn IsMenuItem<Wry>)
        .collect();
    interval_refs.push(&int_sep);
    interval_refs.push(&int_custom);
    let interval_sub = Submenu::with_items(
        app,
        format!("Reminder every {} min", settings.interval_min),
        true,
        &interval_refs,
    )?;

    // Snooze submenu
    let snooze_items: Vec<CheckMenuItem<Wry>> = SNOOZE_OPTIONS
        .iter()
        .map(|minutes| {
            CheckMenuItem::with_id(
                app,
                format!("snz_{minutes}"),
                format!("{minutes} min"),
                true,
                settings.snooze_min == *minutes,
                None::<&str>,
            )
        })
        .collect::<tauri::Result<_>>()?;
    let snz_sep = PredefinedMenuItem::separator(app)?;
    let snz_custom = MenuItem::with_id(app, "custom_snz", "Custom...", true, None::<&str>)?;
    let mut snooze_refs: Vec<&dyn IsMenuItem<Wry>> = snooze_items
        .iter()
        .map(|i| i as &dyn IsMenuItem<Wry>)
        .collect();
    snooze_refs.push(&snz_sep);
    snooze_refs.push(&snz_custom);
    let snooze_sub = Submenu::with_items(
        app,
        format!("Snooze for {} min", settings.snooze_min),
        true,
        &snooze_refs,
    )?;

    // Theme submenu
    let theme_items: Vec<CheckMenuItem<Wry>> = THEME_IDS
        .iter()
        .map(|id| {
            CheckMenuItem::with_id(
                app,
                format!("thm_{id}"),
                theme_label(id),
                true,
                settings.theme_id == *id,
                None::<&str>,
            )
        })
        .collect::<tauri::Result<_>>()?;
    let theme_refs: Vec<&dyn IsMenuItem<Wry>> = theme_items
        .iter()
        .map(|i| i as &dyn IsMenuItem<Wry>)
        .collect();
    let theme_sub = Submenu::with_items(
        app,
        format!("Theme: {}", theme_label(&settings.theme_id)),
        true,
        &theme_refs,
    )?;

    let name_label = if settings.name.is_empty() {
        "Name: not set".to_string()
    } else {
        format!("Name: {}", settings.name)
    };
    let name_item = MenuItem::with_id(app, "name", name_label, true, None::<&str>)?;
    let pause_item =
        CheckMenuItem::with_id(app, "pause", "Pause reminders", true, paused, None::<&str>)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit Hydrate Buddy", true, None::<&str>)?;

    let all_refs: Vec<&dyn IsMenuItem<Wry>> = vec![
        &drink,
        &settings_item,
        &sep1,
        &interval_sub,
        &snooze_sub,
        &theme_sub,
        &name_item,
        &pause_item,
        &sep2,
        &quit_item,
    ];
    Menu::with_items(app, &all_refs)
}

fn rebuild_tray(app: &AppHandle) {
    let Some(tray) = app.tray_by_id("main") else {
        return;
    };
    let state = app.state::<AppState>();
    let settings = state.settings.lock().clone();
    if let Err(e) = tray.set_icon(Some(tray_icon_for(&settings.theme_id))) {
        log::warn!("failed to set tray icon: {e}");
    }
    match build_menu(app) {
        Ok(menu) => {
            if let Err(e) = tray.set_menu(Some(menu)) {
                log::warn!("failed to set tray menu: {e}");
            }
        }
        Err(e) => log::warn!("failed to build tray menu: {e}"),
    }
    update_tray_tooltip(app);
}

fn update_tray_tooltip(app: &AppHandle) {
    let Some(tray) = app.tray_by_id("main") else {
        return;
    };
    let state = app.state::<AppState>();
    let paused = *state.paused.lock();
    let tip = if paused {
        "Hydrate Buddy — paused".to_string()
    } else {
        let next = *state.next_reminder_at.lock();
        let mins = (((next - now_ms()) / 60_000).max(0)).max(0);
        let s = state.settings.lock().clone();
        format!(
            "Hydrate Buddy — {} — next nudge in ~{} min",
            theme_label(&s.theme_id),
            mins
        )
    };
    let _ = tray.set_tooltip(Some(&tip));
}

// ---- Core reminder flow --------------------------------------------------
fn trigger_reminder(app: &AppHandle) {
    let state = app.state::<AppState>();
    if *state.paused.lock() {
        return;
    }
    if !is_within_active_hours() {
        return;
    }
    let Some(win) = app.get_webview_window("reminder") else {
        return;
    };
    if win.is_visible().unwrap_or(false) {
        return;
    }
    {
        let s = state.settings.lock().clone();
        let mut next = state.next_reminder_at.lock();
        *next = now_ms() + reminder_delay_ms(&s);
    }
    position_window(app);
    if let Err(e) = win.show() {
        log::warn!("failed to show reminder window: {e}");
    }
    if let Err(e) = win.set_focus() {
        log::warn!("failed to focus reminder window: {e}");
    }
    if let Err(e) = win.set_always_on_top(true) {
        log::warn!("failed to keep reminder on top: {e}");
    }
    let payload = reminder_payload(
        state.settings.lock().clone(),
        *state.consecutive_snoozes.lock(),
    );
    if let Err(e) = app.emit_to("reminder", "reminder:show", &payload) {
        log::warn!("failed to emit reminder:show: {e}");
    }
    update_tray_tooltip(app);
}

fn tick(app: &AppHandle) {
    let state = app.state::<AppState>();
    if *state.paused.lock() {
        return;
    }
    let Some(win) = app.get_webview_window("reminder") else {
        return;
    };
    if win.is_visible().unwrap_or(false) {
        return;
    }
    if !is_within_active_hours() {
        return;
    }
    let next = *state.next_reminder_at.lock();
    if now_ms() >= next {
        trigger_reminder(app);
    }
}

fn apply_settings(app: &AppHandle, patch: SettingsPatch, reschedule: bool) -> Settings {
    let state = app.state::<AppState>();
    {
        let mut s = state.settings.lock();
        if let Some(v) = patch.name {
            let trimmed = v.trim();
            s.name = trimmed.chars().take(24).collect();
        }
        if let Some(v) = patch.interval_min {
            s.interval_min = v.clamp(1, 240);
        }
        if let Some(v) = patch.snooze_min {
            s.snooze_min = v.clamp(1, 120);
        }
        if let Some(v) = patch.theme_id {
            s.theme_id = resolve_theme(&v);
        }
    }
    let snapshot = state.settings.lock().clone();
    save_config(app, &snapshot);
    if reschedule {
        let mut next = state.next_reminder_at.lock();
        *next = now_ms() + reminder_delay_ms(&snapshot);
    }
    let _ = app.emit("settings:updated", &snapshot);
    rebuild_tray(app);
    snapshot
}

// ---- IPC commands --------------------------------------------------------
#[tauri::command]
fn settings_get(state: State<'_, AppState>) -> Settings {
    state.settings.lock().clone()
}

#[tauri::command]
fn settings_save(value: SettingsPatch, app: AppHandle) -> Settings {
    let current = {
        let state = app.state::<AppState>();
        let current = state.settings.lock().clone();
        current
    };
    let reschedule = patch_changes_interval(&current, &value);
    apply_settings(&app, value, reschedule)
}

#[tauri::command]
fn reminder_yes(app: AppHandle) {
    let state = app.state::<AppState>();
    let s = state.settings.lock().clone();
    *state.consecutive_snoozes.lock() = 0;
    *state.next_reminder_at.lock() = now_ms() + reminder_delay_ms(&s);
    update_tray_tooltip(&app);
}

#[tauri::command]
fn reminder_snooze(app: AppHandle) -> SnoozeOutcome {
    let state = app.state::<AppState>();
    let s = state.settings.lock().clone();
    let snooze_count = {
        let mut count = state.consecutive_snoozes.lock();
        *count = next_snooze_count(*count);
        *count
    };
    let forced_drink = should_force_drink(snooze_count);
    if !forced_drink {
        *state.next_reminder_at.lock() = now_ms() + snooze_delay_ms(&s);
    }
    update_tray_tooltip(&app);
    SnoozeOutcome {
        snooze_count,
        forced_drink,
        snooze_min: s.snooze_min,
    }
}

#[tauri::command]
fn reminder_hide(app: AppHandle) {
    if let Some(win) = app.get_webview_window("reminder") {
        let _ = win.hide();
    }
}

#[tauri::command]
fn settings_close(app: AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.close();
    }
}

fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
    let id = event.id().as_ref();
    match id {
        "drink" => trigger_reminder(app),
        "settings" | "name" => open_settings(app),
        "custom_int" => open_settings_with_focus(app, Some("interval")),
        "custom_snz" => open_settings_with_focus(app, Some("snooze")),
        "pause" => {
            let state = app.state::<AppState>();
            let mut paused = state.paused.lock();
            *paused = !*paused;
            let now_paused = *paused;
            drop(paused);
            if now_paused {
                if let Some(win) = app.get_webview_window("reminder") {
                    let _ = win.hide();
                }
            } else {
                let s = state.settings.lock().clone();
                let mut next = state.next_reminder_at.lock();
                *next = now_ms() + reminder_delay_ms(&s);
            }
            rebuild_tray(app);
        }
        "quit" => app.exit(0),
        other => {
            if let Some(rest) = other.strip_prefix("int_") {
                if let Ok(n) = rest.parse::<i64>() {
                    apply_settings(
                        app,
                        SettingsPatch {
                            interval_min: Some(n),
                            ..Default::default()
                        },
                        true,
                    );
                }
            } else if let Some(rest) = other.strip_prefix("snz_") {
                if let Ok(n) = rest.parse::<i64>() {
                    apply_settings(
                        app,
                        SettingsPatch {
                            snooze_min: Some(n),
                            ..Default::default()
                        },
                        false,
                    );
                }
            } else if let Some(rest) = other.strip_prefix("thm_") {
                apply_settings(
                    app,
                    SettingsPatch {
                        theme_id: Some(rest.to_string()),
                        ..Default::default()
                    },
                    false,
                );
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn configure_mac_menu_bar_mode(app: &AppHandle) {
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
}

#[cfg(not(target_os = "macos"))]
fn configure_mac_menu_bar_mode(_app: &AppHandle) {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir { file_name: None }),
                ])
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            settings_get,
            settings_save,
            reminder_yes,
            reminder_snooze,
            reminder_hide,
            settings_close,
        ])
        .on_menu_event(handle_menu_event)
        .setup(|app| {
            let app_handle = app.handle().clone();
            configure_mac_menu_bar_mode(&app_handle);

            let settings = load_config(&app_handle);
            let next_reminder_at = now_ms()
                + if is_within_active_hours() {
                    GREETING_DELAY_MS
                } else {
                    reminder_delay_ms(&settings)
                };

            app.manage(AppState {
                settings: Mutex::new(settings),
                paused: Mutex::new(false),
                next_reminder_at: Mutex::new(next_reminder_at),
                consecutive_snoozes: Mutex::new(0),
            });

            create_reminder_window(&app_handle)?;

            if let Err(e) = TrayIconBuilder::with_id("main")
                .icon(tray_icon_for("default"))
                .tooltip("Hydrate Buddy")
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        trigger_reminder(tray.app_handle());
                    }
                })
                .build(app)
            {
                log::error!("failed to build tray icon: {e}");
            }

            rebuild_tray(&app_handle);

            // Background scheduler: re-checks wall-clock every TICK_MS so it
            // survives sleep/wake (mirrors the Electron setInterval(tick)).
            let sched_handle = app_handle.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_millis(TICK_MS));
                tick(&sched_handle);
            });

            // First nudge shortly after launch, within active hours.
            let greet_handle = app_handle.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(GREETING_DELAY_MS as u64 + 300));
                tick(&greet_handle);
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snooze_counter_forces_drink_on_third_snooze() {
        let first = next_snooze_count(0);
        let second = next_snooze_count(first);
        let third = next_snooze_count(second);

        assert_eq!(first, 1);
        assert_eq!(second, 2);
        assert_eq!(third, FORCED_DRINK_SNOOZE_LIMIT);
        assert!(!should_force_drink(first));
        assert!(!should_force_drink(second));
        assert!(should_force_drink(third));
    }

    #[test]
    fn snooze_counter_stays_capped_after_limit() {
        assert_eq!(
            next_snooze_count(FORCED_DRINK_SNOOZE_LIMIT),
            FORCED_DRINK_SNOOZE_LIMIT
        );
        assert!(should_force_drink(FORCED_DRINK_SNOOZE_LIMIT));
    }

    #[test]
    fn reminder_payload_marks_forced_drink_from_snooze_count() {
        let payload = reminder_payload(Settings::default(), FORCED_DRINK_SNOOZE_LIMIT);

        assert!(payload.forced_drink);
        assert_eq!(payload.snooze_count, FORCED_DRINK_SNOOZE_LIMIT);
        assert_eq!(payload.theme_id, "default");
    }

    #[test]
    fn settings_patch_reschedules_only_when_interval_changes() {
        let settings = Settings::default();

        assert!(!patch_changes_interval(
            &settings,
            &SettingsPatch {
                name: Some("Ada".to_string()),
                ..Default::default()
            }
        ));
        assert!(!patch_changes_interval(
            &settings,
            &SettingsPatch {
                interval_min: Some(DEFAULT_INTERVAL_MIN),
                ..Default::default()
            }
        ));
        assert!(patch_changes_interval(
            &settings,
            &SettingsPatch {
                interval_min: Some(1),
                ..Default::default()
            }
        ));
    }
}
