// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/*use std::error::Error;*/

mod simple_log;

slint::include_modules!();

#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {

    log!("videofinder started!");

    slint::android::init(app).unwrap();

    log!("slint::android initialized!");

    slint::slint!{
        export component MainWindow inherits Window {
            Text { text: "Hello World"; }
        }
    }
    log!("slint code run");

    MainWindow::new().unwrap().run().unwrap();
/*
    let ui = AppWindow::new()?;

    ui.on_request_increase_value({
        let ui_handle = ui.as_weak();
        move || {
            let ui = ui_handle.unwrap();
            ui.set_counter(ui.get_counter() + 1);
        }
    });

    ui.run()?;

    Ok(())
*/
}
