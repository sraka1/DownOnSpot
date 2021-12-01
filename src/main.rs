#![feature(async_closure)]
#[macro_use]
extern crate log;

mod converter;
mod downloader;
mod error;
mod server;
mod cli;
mod settings;
mod spotify;
mod tag;

use colored::Colorize;
use downloader::{Downloader};
use server::websocket::{self, Websocket};
use settings::Settings;
use spotify::Spotify;
use std::{env::{args}, sync::Arc};

#[cfg(not(windows))]
#[tokio::main]
async fn main() {
	start().await;
}

#[cfg(windows)]
#[tokio::main]
async fn main() {
	use colored::control;

	//backwards compatibility.
	if control::set_virtual_terminal(true).is_ok() {};
	start().await;
}


async fn start() {
	let settings = match Settings::load().await {
		Ok(settings) => {
			println!(
				"{} {}.",
				"Settings successfully loaded.\nContinuing with spotify account:".green(),
				settings.username
			);
			settings
		}
		Err(e) => {
			println!(
				"{} {}...",
				"Settings could not be loaded, because of the following error:".red(),
				e
			);
			let default_settings = Settings::new("username", "password", "client_id", "secret");
			match default_settings.save().await {
				Ok(_) => {
					println!(
						"{}",
						"..but default settings have been created successfully. Edit them and run the program again.".green()
					);
				}
				Err(e) => {
					println!(
						"{} {}",
						"..and default settings could not be written:".red(),
						e
					);
				}
			};
			return;
		}
	};

	let spotify = match Spotify::new(
		&settings.username,
		&settings.password,
		&settings.client_id,
		&settings.client_secret,
	)
	.await
	{
		Ok(spotify) => {
			println!("{}", "Login succeeded.".green());
			spotify
		}
		Err(e) => {
			println!(
				"{} {}",
				"Login failed, possibly due to invalid credentials or settings:".red(),
				e
			);
			return;
		}
	};

	let downloader = Downloader::new(settings.downloader, spotify);

	if settings.websocket.enabled {
		websocket::start(settings.websocket.ip ,settings.websocket.port, Arc::new(downloader));
		return;
	}

	let args: Vec<String> = args().collect();
	if args.len() <= 1 {
		cli::print_usage(args[0].clone());
		return;
	}
	
	let input = args[1..].join(" ");
	match downloader.handle_input(&input).await {
		Ok(search_results) => {
			if let Some(search_results) = search_results{
				let result = cli::select_from_search(search_results);
				
				if let Err(e) = downloader
					.add_uri(&format!("spotify:track:{}", result.track_id))
					.await
				{
					error!(
						"{}",
						format!(
							"{}: {}",
							"Track could not be added to download queue.".red(),
							e
						)
					);
					return;
				}
			}
			println!("s");
			cli::print_download_state(&downloader, settings.refresh_ui_seconds).await;
		}
		Err(e) => {
			error!("{} {}", "Handling input failed:".red(), e)
		}
	}
}