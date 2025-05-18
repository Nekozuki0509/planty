use common::{Config, Plant};
use dotenvy::dotenv;
use poise::serenity_prelude::{self as serenity, ChannelId, Color, ComponentInteractionDataKind, CreateActionRow, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse, CreateInteractionResponseMessage, CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption, EditInteractionResponse, EditMessage, EventHandler, Ready, Timestamp};
use poise::CreateReply;
use std::sync::Arc;
use surrealdb::{engine::remote::ws::Ws, opt::auth::Root, Surreal};

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
    let mut an = vec![String::from("\ndate                voltage")];
    let mut opts = vec![CreateSelectMenuOption::new("Page 1", "1")];
    for i in plants {
        let parse = format!("\n{:} {:.2}V", i.date.format("%Y/%m/%d %H:%M:%S"), i.voltage);
        if an.last().unwrap().len() + parse.len() > 3500 {
            an.push(String::from(String::from("\ndate                voltage")));
            opts.push(CreateSelectMenuOption::new(format!("Page {}", opts.len()+1), (opts.len()+1).to_string()));
        }

        let index = an.len()-1;
        an[index].push_str(&parse);
    }

    let rh = ctx.send(CreateReply::default()
        .embed(CreateEmbed::new()
                   .title(&config.table)
                   .description(&an[0])
                   .color(Color::BLUE)
                   .footer(CreateEmbedFooter::new(format!("1 / {} page", an.len())))
                   .timestamp(Timestamp::now()))
        .components(vec![CreateActionRow::SelectMenu(CreateSelectMenu::new("menu", CreateSelectMenuKind::String {options: opts.clone()}))])
    ).await.unwrap();

    let m = rh.message().await.unwrap();

    loop {
        let mi = m.await_component_interaction(&ctx).await.unwrap();

        let num = match &mi.data.kind {
            ComponentInteractionDataKind::StringSelect { values } => {
                values[0].parse::<i32>().unwrap()
            }

            _ => unreachable!()
        };
        
        let response = CreateInteractionResponse::UpdateMessage(
            CreateInteractionResponseMessage::new()
                .embed(
                    CreateEmbed::new()
                        .title(&config.table)
                        .description(&an[(num-1) as usize])
                        .color(Color::BLUE)
                        .footer(CreateEmbedFooter::new(format!("{} / {} page", num, an.len())))
                        .timestamp(Timestamp::now())
                ).components(vec![CreateActionRow::SelectMenu(CreateSelectMenu::new("menu", CreateSelectMenuKind::String {options: opts.clone()}))])
        );
        
        mi.create_response(ctx, response).await.unwrap();
    }
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

// async fn send_message_embed(ctx: &Context<'_>, embed: CreateEmbed, action: Vec<CreateActionRow>) -> Message {
//     send_message(ctx., CreateMessage::new()
//         .embed(embed)
//         .components(action)
//     ).await.unwrap()
// }