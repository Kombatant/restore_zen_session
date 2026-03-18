use clap::Parser;
use zen_session_restore::{
    cli::{self, Cli},
    gui,
};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    if cli.command.is_some() {
        cli::run(cli)
    } else {
        gui::run(cli.profile, cli.about_dialog)
    }
}
