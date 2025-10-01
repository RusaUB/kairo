use tauri::{menu::{Menu, MenuItem}, tray::TrayIconBuilder};

mod cred;
mod db;


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let active_agent = MenuItem::with_id(app, "active_agent", "Active Agent", true, None::<&str>)?;
            let generate_response = MenuItem::with_id(app, "generate_response", "Generate Response", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&active_agent, &generate_response, &quit])?;

            let _tray = TrayIconBuilder::new()
                .tooltip("Kairo")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(move |app, event| {
                    match event.id.as_ref() {
                        "quit" => {
                            app.exit(0);
                        }
                        "hide" => {
                            todo!()
                        }
                        "show" => {
                            todo!()
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            db::add_llmprovider,
            db::get_llmprovider,
            db::get_all_llmproviders,
            
            db::add_agent,
            db::get_agent,
            db::get_all_agents,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
