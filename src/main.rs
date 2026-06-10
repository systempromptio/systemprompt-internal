use systemprompt_astound as _;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    Box::pin(systemprompt_astound::cli::run()).await
}
