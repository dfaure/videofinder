fn main() {
    let config = slint_build::CompilerConfiguration::new().with_style("fluent-light".into());

    // material-light: too big
    // fluent-light: clean, very square, blue highlight below the lineedit, blue selection
    // cupertino-light: even smaller, clean too, blue highlight around the lineedit
    // cosmic-light: too gray
    //
    // Changing the config rebuilds many things; try SLINT_STYLE instead, on Linux
    // (not an option on Android...)

    slint_build::compile_with_config("ui/app-window.slint", config).expect("Slint build failed");
}
