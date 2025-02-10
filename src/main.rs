#[macro_use]
extern crate rocket;

use std::{net::Ipv4Addr, sync::Arc, time::Duration};

use rocket::{
    serde::{json::Json, Deserialize, Serialize},
    Config, State,
};
use rumqttc::{AsyncClient, MqttOptions, QoS};

// use serde::{Deserialize, Serialize};
use tokio::{sync::Mutex, task};

#[derive(Serialize, Deserialize)]
pub struct JsonTemplate {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<Vec<Text>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    textbox: Option<Vec<Text>>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum Text {
    Integer(i64),

    String(String),
}

#[derive(Clone)]
pub struct SharedData {
    pub n: Arc<Mutex<u8>>,
    pub msg: Arc<Mutex<String>>,
}

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[get("/samla.json")]
async fn samla(state: &State<SharedData>) -> Json<Vec<JsonTemplate>> {
    // 296x152
    let data: Vec<JsonTemplate> = vec![
        JsonTemplate {
            text: Some(vec![
                Text::Integer(147),
                Text::Integer(15),
                Text::String(format!("n={}", state.n.lock().await)),
                Text::String("fonts/Signika-SB.ttf".to_string()),
                Text::Integer(1),
                Text::Integer(1),
                Text::Integer(30),
            ]),
            textbox: None,
        },
        JsonTemplate {
            textbox: Some(vec![
                Text::Integer(4),
                Text::Integer(68),
                Text::Integer(290),
                Text::Integer(50),
                Text::String(state.msg.lock().await.to_string()),
                Text::String("fonts/calibrib30.vlw".to_string()),
                Text::Integer(1),
                // Text::Integer(1),
                // Text::Integer(24),
            ]),
            text: None,
        },
    ];

    Json(data)
}

#[launch]
async fn rocket() -> _ {
    let data = SharedData {
        msg: Arc::new(Mutex::new("".to_string())),
        n: Arc::new(Mutex::new(0)),
    };

    let mut mqttoptions = MqttOptions::new("mqtt", "revspace.nl", 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
    client
        .subscribe("revspace/doorduino/checked-in", QoS::AtMostOnce)
        .await
        .unwrap();

    client
        .subscribe("revspace/sting/samla", QoS::AtMostOnce)
        .await
        .unwrap();

    let data2 = data.clone();
    task::spawn(async move {
        loop {
            let event = eventloop.poll().await;
            if let Ok(rumqttc::Event::Incoming(rumqttc::Packet::Publish(publish))) = &event {
                match publish.topic.as_str() {
                    "revspace/sting/samla" => {
                        let payload = std::str::from_utf8(&publish.payload).unwrap().to_string();

                        let mut lock = data2.msg.lock().await;
                        *lock = payload;
                    }
                    "revspace/doorduino/checked-in" => {
                        let payload = std::str::from_utf8(&publish.payload)
                            .unwrap()
                            .parse::<u8>()
                            .unwrap();

                        let mut lock = data2.n.lock().await;
                        *lock = payload;
                    }
                    _ => {}
                }
            }
        }
    });

    let config = Config {
        port: 80,
        address: Ipv4Addr::new(0, 0, 0, 0).into(),
        cli_colors: false,
        ..Config::release_default()
    };
    rocket::build()
        .mount("/", routes![index, samla])
        .configure(config)
        .manage(data)
}
