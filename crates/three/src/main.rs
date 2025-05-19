use common::{Config, Plant};
use dotenvy::dotenv;
use poise::serenity_prelude::{self as serenity};
use poise::CreateReply;
use serenity::{ChannelId, Color, ComponentInteractionDataKind, CreateActionRow, CreateAttachment, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse, CreateInteractionResponseMessage, CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption, EventHandler, Ready, Timestamp};
use std::sync::Arc;
use chrono::{DateTime, Local};
use plotters::backend::BitMapBackend;
use plotters::chart::ChartBuilder;
use plotters::prelude::{IntoDrawingArea, IntoFont, LineSeries, RED, WHITE};
use surrealdb::{engine::remote::ws::Ws, opt::auth::Root, Surreal};
use tokio::fs::File;

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
async fn graph(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let data = ctx.data();
    let config = data.config.clone();
    let db = data.con.clone();
    let mut plants: Vec<Plant> = db.select(&config.table).await?;
    plants.sort_by(|x, x1| {x.date.cmp(&x1.date)});

    {
        let xs: Vec<DateTime<Local>> = plants.iter().map(|x| x.date).collect();
        let ys: Vec<f64> = plants.iter().map(|x| x.voltage).collect::<Vec<f64>>();

        let root = BitMapBackend::new("plot.png", (1080, 720)).into_drawing_area();
        root.fill(&WHITE)?;

        let (y_min, y_max) = ys.iter()
            .fold(
                (0.0 / 0.0, 0.0 / 0.0),
                |(m, n), v| (v.min(m), v.max(n)),
            );

        let mut chart = ChartBuilder::on(&root)
            .caption(&config.table, ("sans-serif", 20).into_font())
            .margin(10)
            .x_label_area_size(16)
            .y_label_area_size(42)
            .build_cartesian_2d(
                *xs.first().unwrap()..*xs.last().unwrap(),
                y_min..y_max,
            )?;

        chart.configure_mesh().x_label_formatter(&|x: &DateTime<Local>| x.format("%Y/%m/%d %H:%M").to_string()).draw()?;
        let line_series = LineSeries::new(
            xs.iter()
                .zip(ys.iter())
                .map(|(x, y)| (*x, *y)),
            &RED,
        );
        chart.draw_series(line_series)?;
        root.present()?;
    }
    
    ctx.send(CreateReply::default().attachment(CreateAttachment::file(&File::open("plot.png").await?, format!("{}.png", &config.table)).await?)).await?;
    
    Ok(())
}

#[poise::command(slash_command, prefix_command)]
async fn select(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    let data = ctx.data();
    let config = data.config.clone();
    let db = data.con.clone();
    let mut plants: Vec<Plant> = db.select(&config.table).await?;
    plants.sort_by(|x, x1| {x1.date.cmp(&x.date)});
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
                values[0].parse::<isize>().unwrap()
            }

            _ => unreachable!()
        };
        
        let response = CreateInteractionResponse::UpdateMessage(
            CreateInteractionResponseMessage::new()
                .embed(
                    CreateEmbed::new()
                        .title(&config.table)
                        .description(&an[(&num-1) as usize])
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
            commands: vec![age(), select(), graph()],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {config, con: db.clone()})
            })
        })
        .build();

    let client = serenity::Client::builder(std::env::var("DISCORD_TOKEN").unwrap(), intents).event_handler(Handler).framework(framework).await;

    client.unwrap().start().await?;

    Ok(())
}