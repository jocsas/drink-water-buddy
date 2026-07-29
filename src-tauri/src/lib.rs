// Hydrate Buddy — Tauri port of the Electron app.
// Equivalent of main.js: config persistence, scheduler, tray, windows, IPC.

use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::Utc;
use chrono_tz::Asia::Kolkata;
use serde::{Deserialize, Serialize};
use tauri::{
    image::Image, menu::CheckMenuItem, menu::IsMenuItem, menu::Menu, menu::MenuEvent,
    menu::MenuItem, menu::PredefinedMenuItem, menu::Submenu, AppHandle,
    Emitter, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindowBuilder, Wry,
};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

// ---- Configuration -------------------------------------------------------
const ACTIVE_START_HOUR: u32 = 10;
const ACTIVE_END_HOUR: u32 = 23;
const DEFAULT_INTERVAL_MIN: i64 = 45;
const DEFAULT_SNOOZE_MIN: i64 = 10;
const GREETING_DELAY_MS: i64 = 6000;
const TICK_MS: u64 = 30000;

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

fn clamp_num(value: i64, _fallback: i64, min: i64, max: i64) -> i64 {
    value.clamp(min, max)
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
    if let Some(path) = config_file(app) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json = serde_json::json!({
            "name": settings.name,
            "intervalMin": settings.interval_min,
            "snoozeMin": settings.snooze_min,
            "themeId": settings.theme_id,
        });
        if let Ok(text) = serde_json::to_string_pretty(&json) {
            let _ = std::fs::write(path, text);
        }
    }
}

// ---- Shared app state ----------------------------------------------------
struct AppState {
    settings: Mutex<Settings>,
    paused: Mutex<bool>,
    next_reminder_at: Mutex<i64>,
}

fn reminder_delay_ms(s: &Settings) -> i64 {
    s.interval_min * 60_000
}
fn snooze_delay_ms(s: &Settings) -> i64 {
    s.snooze_min * 60_000
}

fn is_within_active_hours() -> bool {
    let hour: u32 = Utc::now()
        .with_timezone(&Kolkata)
        .format("%H")
        .to_string()
        .parse()
        .unwrap_or(0);
    hour >= ACTIVE_START_HOUR && hour < ACTIVE_END_HOUR
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
        let y = (area.position.y + area.size.height as i32) as f64 - win_h_phys - EDGE_MARGIN as f64;
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
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
        .inner_size(430.0, 470.0)
        .title("Hydrate Buddy Settings")
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .build();
}

// ---- Tray ----------------------------------------------------------------
fn tray_icon_for(theme_id: &str) -> Image<'static> {
    let bytes: &[u8] = match theme_id {
        "baby-yoda" => include_bytes!("../icons/tray/baby-yoda.png"),
        "darth-vader" => include_bytes!("../icons/tray/darth-vader.png"),
        "wizard" => include_bytes!("../icons/tray/wizard.png"),
        _ => include_bytes!("../icons/tray/default.png"),
    };
    let decoded = image::load_from_memory(bytes).expect("decode tray icon").to_rgba8();
    let (width, height) = decoded.dimensions();
    Image::new_owned(decoded.into_raw(), width, height)
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let paused = *state.paused.lock().unwrap();

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
    let theme_refs: Vec<&dyn IsMenuItem<Wry>> =
        theme_items.iter().map(|i| i as &dyn IsMenuItem<Wry>).collect();
    let theme_sub =
        Submenu::with_items(app, format!("Theme: {}", theme_label(&settings.theme_id)), true, &theme_refs)?;

    let name_label = if settings.name.is_empty() {
        "Name: not set".to_string()
    } else {
        format!("Name: {}", settings.name)
    };
    let name_item = MenuItem::with_id(app, "name", name_label, true, None::<&str>)?;
    let pause_item = CheckMenuItem::with_id(app, "pause", "Pause reminders", true, paused, None::<&str>)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit Hydrate Buddy", true, None::<&str>)?;

    let all_refs: Vec<&dyn IsMenuItem<Wry>> = vec![
        &drink, &settings_item, &sep1, &interval_sub, &snooze_sub, &theme_sub, &name_item,
        &pause_item, &sep2, &quit_item,
    ];
    Menu::with_items(app, &all_refs)
}

fn rebuild_tray(app: &AppHandle) {
    let Some(tray) = app.tray_by_id("main") else {
        return;
    };
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let _ = tray.set_icon(Some(tray_icon_for(&settings.theme_id)));
    if let Ok(menu) = build_menu(app) {
        let _ = tray.set_menu(Some(menu));
    }
    update_tray_tooltip(app);
}

fn update_tray_tooltip(app: &AppHandle) {
    let Some(tray) = app.tray_by_id("main") else {
        return;
    };
    let state = app.state::<AppState>();
    let paused = *state.paused.lock().unwrap();
    let tip = if paused {
        "Hydrate Buddy — paused".to_string()
    } else {
        let next = *state.next_reminder_at.lock().unwrap();
        let mins = (((next - now_ms()) / 60_000).max(0)).max(0);
        let s = state.settings.lock().unwrap().clone();
        format!("Hydrate Buddy — {} — next nudge in ~{} min", theme_label(&s.theme_id), mins)
    };
    let _ = tray.set_tooltip(Some(&tip));
}

// ---- Core reminder flow --------------------------------------------------
fn trigger_reminder(app: &AppHandle) {
    let state = app.state::<AppState>();
    if *state.paused.lock().unwrap() {
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
        let s = state.settings.lock().unwrap().clone();
        let mut next = state.next_reminder_at.lock().unwrap();
        *next = now_ms() + reminder_delay_ms(&s);
    }
    position_window(app);
    let _ = win.show();
    let _ = win.set_always_on_top(true);
    let payload = state.settings.lock().unwrap().clone();
    let _ = app.emit("reminder:show", &payload);
    update_tray_tooltip(app);
}

fn tick(app: &AppHandle) {
    let state = app.state::<AppState>();
    if *state.paused.lock().unwrap() {
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
    let next = *state.next_reminder_at.lock().unwrap();
    if now_ms() >= next {
        trigger_reminder(app);
    }
}

fn apply_settings(app: &AppHandle, patch: SettingsPatch, reschedule: bool) -> Settings {
    let state = app.state::<AppState>();
    {
        let mut s = state.settings.lock().unwrap();
        if let Some(v) = patch.name {
            let trimmed = v.trim();
            s.name = trimmed.chars().take(24).collect();
        }
        if let Some(v) = patch.interval_min {
            s.interval_min = clamp_num(v, DEFAULT_INTERVAL_MIN, 1, 240);
        }
        if let Some(v) = patch.snooze_min {
            s.snooze_min = clamp_num(v, DEFAULT_SNOOZE_MIN, 1, 120);
        }
        if let Some(v) = patch.theme_id {
            s.theme_id = resolve_theme(&v);
        }
    }
    let snapshot = state.settings.lock().unwrap().clone();
    save_config(app, &snapshot);
    if reschedule {
        let mut next = state.next_reminder_at.lock().unwrap();
        *next = now_ms() + reminder_delay_ms(&snapshot);
    }
    let _ = app.emit("settings:updated", &snapshot);
    rebuild_tray(app);
    snapshot
}

// ---- IPC commands --------------------------------------------------------
#[tauri::command]
fn settings_get(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn settings_save(value: SettingsPatch, app: AppHandle) -> Settings {
    apply_settings(&app, value, false)
}

#[tauri::command]
fn reminder_yes(app: AppHandle) {
    let state = app.state::<AppState>();
    let s = state.settings.lock().unwrap().clone();
    *state.next_reminder_at.lock().unwrap() = now_ms() + reminder_delay_ms(&s);
    update_tray_tooltip(&app);
}

#[tauri::command]
fn reminder_snooze(app: AppHandle) {
    let state = app.state::<AppState>();
    let s = state.settings.lock().unwrap().clone();
    *state.next_reminder_at.lock().unwrap() = now_ms() + snooze_delay_ms(&s);
    update_tray_tooltip(&app);
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
        "settings" | "custom_int" | "custom_snz" | "name" => open_settings(app),
        "pause" => {
            let state = app.state::<AppState>();
            let mut paused = state.paused.lock().unwrap();
            *paused = !*paused;
            let now_paused = *paused;
            drop(paused);
            if now_paused {
                if let Some(win) = app.get_webview_window("reminder") {
                    let _ = win.hide();
                }
            } else {
                let s = state.settings.lock().unwrap().clone();
                let mut next = state.next_reminder_at.lock().unwrap();
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
        .plugin(tauri_plugin_opener::init())
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
            });

            create_reminder_window(&app_handle)?;

            let _ = TrayIconBuilder::with_id("main")
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
                .build(app);

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
