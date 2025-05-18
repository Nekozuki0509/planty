use std::sync::Arc;
use common::{Config, Plant};
use dotenvy::dotenv;
use poise::serenity_prelude::{self as serenity, ChannelId, CreateActionRow, CreateEmbed, CreateMessage, EventHandler, Message, Ready};
use surrealdb::{Surreal, engine::remote::ws::Ws, opt::auth::Root};

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

struct Handler;

struct Data {
    config: Arc<Config>,
    con: Arc<Surreal<surrealdb::engine::remote::ws::Client>>,    
}

#[serenity::async_trait]
impl EventHandler for Handler {
    // Botが起動したときに走る処理
    async fn ready(&self, ctx: poise::serenity_prelude::Context, ready: Ready) {
        ChannelId::new(1373327401286897925)
            .say(&ctx.http, format!("{} is connected!", ready.user.name)).await.expect("TODO: panic message");
        println!("{} is connected!", ready.user.name);
        
    }
}

/// Displays your or another user's account creation date
#[poise::command(slash_command, prefix_command)]
async fn age(
    ctx: Context<'_>,
    #[description = "Selected user"] user: Option<serenity::User>,
) -> Result<(), Error> {
    let u = user.as_ref().unwrap_or_else(|| ctx.author());
    let response = format!("{}'s account was created at {}", u.name, u.created_at());
    ctx.say(response).await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command)]
async fn select(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let data = ctx.data();
    let config = data.config.clone();
    let db = data.con.clone();
    let plants: Vec<Plant> = db.select(&config.table).await?;
    let mut an = String::from("```\ndate                voltage");
    for i in plants {
        let parse = format!("\n{:} {:.2}V", i.date.format("%Y/%M/%D %H:%M:%S"), i.voltage);
        if an.len() + parse.len() > 1500 {
            an += "\n```";
            break;
        }
        
        an += &parse;
    }
    ctx.say(format!("{:?}", an)).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let config = Arc::new(Config {
            host: std::env::var("DB_HOST").unwrap(),
            user: std::env::var("DB_USER").unwrap(),
            password: std::env::var("DB_PASSWORD").unwrap(),
            namespace: std::env::var("DB_NAMESPACE").unwrap(),
            database: std::env::var("DB_NAME").unwrap(),
            table: std::env::var("TABLE_NAME").unwrap(),
        });

    dbg!(&config);

    let db = Arc::new(Surreal::new::<Ws>(&config.host).await?);

    // Signin as a namespace, database, or root user
    db.signin(Root {
        username: &config.user,
        password: &config.password,
    })
    .await?;

    println!("signined");

    // Select a specific namespace / database
    db.use_ns(&config.namespace).use_db(&config.database).await?;

    println!("connected to db");

    let intents = serenity::GatewayIntents::non_privileged();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![age(), select()],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {config: config, con: db.clone()})
            })
        })
        .build();

    let client = serenity::Client::builder(std::env::var("DISCORD_TOKEN").unwrap(), intents).event_handler(Handler).framework(framework).await;

    client.unwrap().start().await?;

    Ok(())
}

async fn send_message_embed(ctx: &poise::serenity_prelude::Context, embed: CreateEmbed, action: Vec<CreateActionRow>) -> Message {
    ChannelId::new(1373327401286897925).send_message(&ctx.http, CreateMessage::new()
        .embed(embed)
        .components(action)
    ).await.unwrap()
}