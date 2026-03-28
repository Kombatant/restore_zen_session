use std::env;

fn main() {
    let qt_include_path = env::var("DEP_QT_INCLUDE_PATH").expect("DEP_QT_INCLUDE_PATH not set");

    let mut config = cpp_build::Config::new();
    for flag in env::var("DEP_QT_COMPILE_FLAGS")
        .expect("DEP_QT_COMPILE_FLAGS not set")
        .split_terminator(';')
    {
        config.flag(flag);
    }

    config.include(qt_include_path).build("src/gui.rs");
}
