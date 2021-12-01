use std::{net::TcpListener, path::MAIN_SEPARATOR, sync::Arc};

use async_std::fs::File;
use colored::Colorize;
use futures::AsyncReadExt;

use serde::{Deserialize, Serialize};
use tungstenite::accept;

use crate::downloader::{DownloadState, Downloader};

#[derive(Deserialize)]
enum RequestAction {
	Search,
	Add,
	Status,
	Download,
}

#[derive(Serialize)]
enum ResponseAction {
	Search,
	Status,
	Download,
	NotFound,
}

#[derive(Deserialize, Serialize)]
struct Message<T> {
	action: T,
	content: String,
}

#[derive(Deserialize, Serialize)]

struct DownloadStatus {
	title: String,
	size: usize,
	read: usize,
}

pub fn start(ip: String, port: u16, downloader: Arc<Downloader>) {
	let server = TcpListener::bind(format!("{}:{}", ip, port)).unwrap();
	for stream in server.incoming() {

		tokio::spawn(async move {
			let mut websocket = accept(stream.unwrap()).unwrap();

			println!("Client connected");

			loop {
				let client_message = match websocket.read_message() {
					Ok(msg) => msg,
					Err(_) => {
						println!("Client disconnected");
						return;
					}
				};

				if !client_message.is_text() {
					println!("Client sent non-text message");
					continue;
				}

				let message = deserialize_message(client_message);

				match message.action {
					RequestAction::Search => {
						match downloader.handle_input(&message.content).await {
							Ok(result) => {
								if let Some(search_results) = result {
									let message = serialize_message(
										ResponseAction::Search,
										serde_json::to_string(&search_results).unwrap(),
									);

									websocket
										.write_message(message)
										.expect("Failed to send search result");
								} else {
									let message =
										serialize_message(ResponseAction::Search, "".to_string());

									websocket
										.write_message(message)
										.expect("Failed to send confirmation to download");
								}
							}
							Err(e) => {
								error!("{}", format!("Error handling input: {}", e).red());
							}
						}
					}
					RequestAction::Add => {
						match downloader
							.add_uri(&format!("spotify:track:{}", &message.content))
							.await
						{
							Ok(_) => {
								println!("Added track to downloads: {}", message.content);
							}
							Err(e) => {
								error!(
									"{}",
									format!("Error adding track to downloads: {}", e).red()
								);
							}
						}
					}
					RequestAction::Status => {
						let mut downloads = vec![];

						for download in downloader.get_downloads().await {
							if let DownloadState::Downloading(r, t) = download.state {
								downloads.push(DownloadStatus {
									title: download.title,
									size: t,
									read: r,
								});
							} else if let DownloadState::Done(_) = download.state {
								downloads.push(DownloadStatus {
									title: download.title,
									size: 0,
									read: 0,
								});
							} else if let DownloadState::Error(e) = download.state {
								println!("{}", e);
							}
						}

						let message = serialize_message(
							ResponseAction::Status,
							serde_json::to_string(&downloads).unwrap(),
						);

						websocket
							.write_message(message)
							.expect("Failed to send status");
					}
					RequestAction::Download => {
						match downloader
							.get_finished_downloads()
							.await
							.into_iter()
							.find(|d| d.track_id == message.content)
						{
							Some(d) => {
								let path = &d.file_path;
								let mut file = match File::open(path).await {
									Ok(f) => f,
									Err(e) => {
										error!("{}", format!("File not found: {}", e).red());
										continue;
									}
								};
								let mut buffer = Vec::new();
								file.read_to_end(&mut buffer).await.unwrap();
								let _ = async_std::fs::remove_file(path).await;

								let filename =
									path.split(MAIN_SEPARATOR).last().unwrap().to_string();
								let message = serialize_message(ResponseAction::Download, filename);
								websocket
									.write_message(message)
									.expect("Failed to send file");

								//send file as binary
								websocket
									.write_message(tungstenite::Message::binary(buffer))
									.expect("Failed to send file");
							}
							None => {
								let message = serialize_message(
									ResponseAction::NotFound,
									"Requested file not available".to_string(),
								);

								websocket
									.write_message(message)
									.expect("Failed to send file");
							}
						};
					}
				}
			}
		});
	}
}

fn deserialize_message(msg: tungstenite::protocol::Message) -> Message<RequestAction> {
	serde_json::from_str(&msg.into_text().unwrap()).expect("Failed to parse message")
}

fn serialize_message(action: ResponseAction, msg: String) -> tungstenite::protocol::Message {
	serde_json::to_string(&Message {
		action,
		content: msg,
	})
	.unwrap()
	.into()
}

#[derive(Serialize, Deserialize)]
pub struct WebsocketConfig {
	pub enabled: bool,
	pub ip: String,
	pub port: u16,
}

impl WebsocketConfig {
	// Create new instance
	pub fn new() -> WebsocketConfig {
		WebsocketConfig {
			enabled: false,
			ip: "127.0.0.1".to_string(),
			port: 8080,
		}
	}
}
