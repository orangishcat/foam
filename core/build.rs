fn main() {
    let config = slint_build::CompilerConfiguration::new()
        .with_library_paths(vivi_ui::import_paths());
    slint_build::compile_with_config("ui/app_window.slint", config)
        .expect("Slint build failed");
}
