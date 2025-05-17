use std::error::Error;
use std::time::Duration;
use chrono::{DateTime, Local};
use dotenvy::dotenv;
use rppal::gpio::Gpio;
use rppal::spi::{Bus, Mode, SlaveSelect, Spi};
use serde::{Deserialize, Serialize};
use surrealdb::{RecordId, Surreal, engine::remote::ws::Ws, opt::auth::Root};
use tokio::{signal, sync::broadcast};

#[derive(Debug)]
struct Config {
    host: String,
    user: String,
    password: String,
    namespace: String,
    database: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Plant {
    voltage: f32,
    date: DateTime<Local>
}

#[derive(Debug, Deserialize)]
struct Record {
    id: RecordId,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (shutdown_tx, mut shutdown_rx) = broadcast::channel::<()>(1);

    let shutdown_trigger = {
        let shutdown_tx = shutdown_tx.clone();
        tokio::spawn(async move {
            signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
            println!("Ctrl+C received! Sending shutdown signal...");
            let _ = shutdown_tx.send(()); // 受信者に通知
        })
    };

    dotenv().ok();

    let config: Config = Config {
        host: std::env::var("DB_HOST").unwrap(),
        user: std::env::var("DB_USER").unwrap(),
        password: std::env::var("DB_PASSWORD").unwrap(),
        namespace: std::env::var("DB_NAMESPACE").unwrap(),
        database: std::env::var("DB_NAME").unwrap(),
    };

    dbg!(&config);

    let db = Surreal::new::<Ws>(&config.host).await?;

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

    // GPIO 17 を HIGH 出力（電源供給）
    let gpio = Gpio::new()?;
    let mut power_pin = gpio.get(17)?.into_output();
    power_pin.set_high();

    // SPI 初期化
    let spi = Spi::new(Bus::Spi0, SlaveSelect::Ss0, 1_000_000, Mode::Mode0)?;

    println!("spi was initialized");

    loop {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(1)) => {
                let tx = [0x06, 0x00, 0x00];
                let mut rx = [0u8; 3];

                spi.transfer(&mut rx, &tx)?;
                let result: u16 = (((rx[1] & 0x0F) as u16) << 8) | (rx[2] as u16);
                let voltage = 3.3 * result as f32 / 4096.0;

                println!("Voltage: {:.3} V", voltage);

                let record: Option<Record> = db
                .create(&config.database)
                .content(Plant {
                    voltage,
                    date: Local::now()
                })
                .await?;

                dbg!(record);
            }

            _ = shutdown_rx.recv() => {
                break;
            }
        }
    }

    power_pin.set_low();

    Ok(())
}
