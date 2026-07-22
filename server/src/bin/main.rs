use kindleaf_server::app::App;
use loco_rs::cli;

#[tokio::main]
async fn main() -> loco_rs::Result<()> {
    #[cfg(feature = "db")]
    {
        return cli::main::<App, migration::Migrator>().await;
    }

    #[cfg(not(feature = "db"))]
    cli::main::<App>().await
}
