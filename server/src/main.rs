use kindergarten_storybook_server::app::App;

#[tokio::main]
async fn main() -> loco_rs::Result<()> {
    loco_rs::cli::main::<App, migration::Migrator>().await
}
