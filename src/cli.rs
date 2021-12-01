use std::{
	borrow::Borrow,
	io::stdin,
	time::{Duration, Instant},
};

use async_std::task;
use colored::Colorize;

use crate::downloader::{DownloadState, Downloader, SearchResult};

pub fn clear() {
	print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
}

pub fn print_usage(filename: String) {
	println!(
		"Usage:\n{} (track_url | album_url | playlist_url | artist_url )",
		filename
	);
	let mut input = String::new();
	stdin().read_line(&mut input).unwrap();
}

pub fn select_from_search(search_results: Vec<SearchResult>) -> SearchResult {
	clear();

	for (i, track) in search_results.iter().enumerate() {
		println!("{}: {} - {}", i + 1, track.author, track.title);
	}
	println!("{}", "Select the track (default: 1): ".green());

	loop {
		let mut input = String::new();
		std::io::stdin()
			.read_line(&mut input)
			.expect("Failed to read line");

		let selection = input.trim().parse::<usize>().unwrap_or(1) - 1;

		if selection > search_results.len() {
			println!("{}", "Invalid selection. Try again or quit (CTRL+C):".red());
			continue;
		}
		search_results[selection].borrow();
	}
}

pub async fn print_download_state(downloader: &Downloader, refresh_rate: u64) {
	clear();
	let refresh = Duration::from_secs(refresh_rate);
	let now = Instant::now();
	let mut time_elapsed: u64;

	'outer: loop {
		clear();
		let mut exit_flag: i8 = 1;

		for download in downloader.get_downloads().await {
			let state = download.state;

			let progress: String;

			if  let DownloadState::Done(_) = state {
				progress = "Done.".to_string();
			} else {
				exit_flag = 0;
				progress = match state {
					DownloadState::Downloading(r, t) => {
						let p = r as f32 / t as f32 * 100.0;
						if p > 100.0 {
							"100%".to_string()
						} else {
							format!("{}%", p as i8)
						}
					}
					DownloadState::Post => "Postprocessing... ".to_string(),
					DownloadState::None => "Preparing... ".to_string(),
					DownloadState::Lock => "Preparing... ".to_string(),
					DownloadState::Error(e) => {
						exit_flag = 1;
						format!("{}", e)
					}
					DownloadState::Done(_) => {
						exit_flag = 1;
						"Impossible state".to_string()
					}
				};
			}

			println!("{:<19}| {}", progress, download.title);
		}
		time_elapsed = now.elapsed().as_secs();
		if exit_flag == 1 {
			break 'outer;
		}

		println!("\nElapsed second(s): {}", time_elapsed);
		task::sleep(refresh).await
	}
	println!("Finished download(s) in {} second(s).", time_elapsed);
}
