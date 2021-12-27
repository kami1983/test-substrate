// This file is part of Substrate.

// Copyright (C) 2017-2021 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Console informant. Prints sync progress and block events. Runs on the calling thread.

use ansi_term::Colour;
use futures::prelude::*;
use futures_timer::Delay;
use log::{debug, info, trace};
use parity_util_mem::MallocSizeOf;
use sc_client_api::{BlockchainEvents, UsageProvider};
use sc_network::NetworkService;
use sc_transaction_pool_api::TransactionPool;
use sp_blockchain::HeaderMetadata;
use sp_runtime::traits::{Block as BlockT, Header};
use std::{collections::VecDeque, fmt::Display, sync::Arc, time::Duration};

mod display;

/// Creates a stream that returns a new value every `duration`.
fn interval(duration: Duration) -> impl Stream<Item = ()> + Unpin {
	futures::stream::unfold((), move |_| Delay::new(duration).map(|_| Some(((), ())))).map(drop)
}

/// The format to print telemetry output in.
#[derive(Clone, Debug)]
pub struct OutputFormat {
	/// Enable color output in logs.
	///
	/// Is enabled by default.
	pub enable_color: bool,
}

impl Default for OutputFormat {
	fn default() -> Self {
		Self { enable_color: true }
	}
}

/// Marker trait for a type that implements `TransactionPool` and `MallocSizeOf` on `not(target_os =
/// "unknown")`.
#[cfg(target_os = "unknown")]
pub trait TransactionPoolAndMaybeMallogSizeOf: TransactionPool {}

/// Marker trait for a type that implements `TransactionPool` and `MallocSizeOf` on `not(target_os =
/// "unknown")`.
#[cfg(not(target_os = "unknown"))]
pub trait TransactionPoolAndMaybeMallogSizeOf: TransactionPool + MallocSizeOf {}

#[cfg(target_os = "unknown")]
impl<T: TransactionPool> TransactionPoolAndMaybeMallogSizeOf for T {}

#[cfg(not(target_os = "unknown"))]
impl<T: TransactionPool + MallocSizeOf> TransactionPoolAndMaybeMallogSizeOf for T {}

/// Builds the informant and returns a `Future` that drives the informant.
pub async fn build<B: BlockT, C>(
	client: Arc<C>,
	network: Arc<NetworkService<B, <B as BlockT>::Hash>>,
	pool: Arc<impl TransactionPoolAndMaybeMallogSizeOf>,
	format: OutputFormat,
) where
	C: UsageProvider<B> + HeaderMetadata<B> + BlockchainEvents<B>,
	<C as HeaderMetadata<B>>::Error: Display,
{
	log::info!("{} Information 的build 并不会被反复触发，只会触发一次",
		ansi_term::Colour::Red.bold().paint("###### information/lib.rs/build"),
	);

	let mut display = display::InformantDisplay::new(format.clone());

	let client_1 = client.clone();
	// interval 创建一个数据流，每个 Duration(5000) 返回一个新的值
	let display_notifications = interval(Duration::from_millis(5000))
		.filter_map(|_| async {
			let status = network.status().await;
			log::info!("{} 创建 display_notifications 的流程不详，过滤掉err的数据，这里只是保留状态是ok的数据流， is_ok? ={:?}",
					   ansi_term::Colour::Red.bold().paint("###### information/lib.rs/build - make display_notifications"),
					   status.is_ok(),
			);
			status.ok()
		})
		.for_each(move |net_status| {
			let info = client_1.usage_info();
			if let Some(ref usage) = info.usage {
				trace!(target: "usage", "Usage statistics: {}", usage);
			} else {
				trace!(
					target: "usage",
					"Usage statistics not displayed as backend does not provide it",
				)
			}
			#[cfg(not(target_os = "unknown"))]
			trace!(
				target: "usage",
				"Subsystems memory [txpool: {} kB]",
				parity_util_mem::malloc_size(&*pool) / 1024,
			);
			log::info!("{} 准备显示状态，这里面调用 display.display(info) 显示：info = client_1.usage_info(): {:?} net_status：这里面就是网络状态，连接几个节点同步速度一类的数据",
					   ansi_term::Colour::Red.bold().paint("###### information/lib.rs/build - make display_notifications"),
					   &info,
			);
			display.display(&info, net_status);
			future::ready(())
		});

	log::info!("{} 如果不是循环触发，这里应该不会进入才对",
			   ansi_term::Colour::Blue.bold().paint("###### information/lib.rs/build - futures::select!"),
	);
	futures::select! {
		() = display_notifications.fuse() => (),
		() = display_block_import(client).fuse() => (),
	};
}

fn display_block_import<B: BlockT, C>(client: Arc<C>) -> impl Future<Output = ()>
where
	C: UsageProvider<B> + HeaderMetadata<B> + BlockchainEvents<B>,
	<C as HeaderMetadata<B>>::Error: Display,
{
	let mut last_best = {
		let info = client.usage_info();
		Some((info.chain.best_number, info.chain.best_hash))
	};

	// Hashes of the last blocks we have seen at import.
	let mut last_blocks = VecDeque::new();
	let max_blocks_to_track = 100;

	log::info!("{} 进入 display_block_import first-last_best 这里是进入区块import的相应方法中 = {:?} 这个实际上是被 task_manager 的函数调用。这个信息只在首区块中才会出现，之后就进入消息的循环监听中。",
			   ansi_term::Colour::Red.bold().paint("######1"),
			   &last_best
	);

	// 这里实际上是一个 client 的数据流监听，循环内容会反复
	// 获取块导入事件流。 不保证每个导入的块都会被触发。
	client.import_notification_stream().for_each(move |n| {
		log::info!("{} 进入 client.import_notification_stream() 就是一个TCP监听，这个实际上是监听结果然后输出用的这里面没有任何的处理。 n = 不打印n， last_best 这个值很重要用来判断是否出现分叉 = {:?}",
				   ansi_term::Colour::Red.bold().paint("###### clinet/information/lib.rs"),
				   // &n, // n 就是 BlockImportNotification 是一个区块导入的通知，那么具体这个通知是哪儿发出的？
			       &last_best,
		);

		log::info!("{} 判断是否出现分叉，首先网络上的数据 n.header.parent_hash() = {:?}，然后本地缓存的数据 last_hash = {:?} 如果他们不一致就会进入分叉处理，实际上是校对本地的缓存。",
				   ansi_term::Colour::Red.bold().paint("###### clinet/information/lib.rs"),
				   n.header.parent_hash(),
				   &last_best,
		);
		// detect and log reorganizations.
		if let Some((ref last_num, ref last_hash)) = last_best {
			if n.header.parent_hash() != last_hash && n.is_new_best {
				// 注意 lowest_common_ancestor 方法，这个方法并不单纯的获取共同祖先这里面他也会更新缓存
				let maybe_ancestor =
					sp_blockchain::lowest_common_ancestor(&*client, last_hash.clone(), n.hash);

				log::info!("{} 判断是否有共同祖先 last_num={:?}, last_hash={:?}, maybe_ancestor = 看不到 ",
						   ansi_term::Colour::Red.bold().paint("######2.1"),
						   last_num,
						   last_hash,
				);
				match maybe_ancestor {
					Ok(ref ancestor) if ancestor.hash != *last_hash => info!(
						"♻️  Reorg on #{},{} to #{},{}, common ancestor #{},{}",
						Colour::Red.bold().paint(format!("{}", last_num)),// 出错的高度
						last_hash, // 本地最后的Hash
						Colour::Green.bold().paint(format!("{}", n.header.number())),
						n.hash, // 网络中的Hash
						Colour::White.bold().paint(format!("{}", ancestor.number)),
						ancestor.hash,
					),
					Ok(info) => {
						log::info!("{} 这是正常的情况 = {:?} ",
								   ansi_term::Colour::Red.bold().paint("######2.2"),
								   &info
						);
					},
					Err(e) => debug!("Error computing tree route: {}", e),
				}
			}
		}

		log::info!("{} 判断 n: BlockImportNotification 是不是 is_new_best = {:?} 如果是那么更新 last_best 否则不更新 ",
			ansi_term::Colour::Red.bold().paint("######2.1"),
			&n.is_new_best,
		);


		if n.is_new_best {
			// 本地的 last_best 实际上就算断线也不会出现不一致的情况，这种情况通常是
			last_best = Some((n.header.number().clone(), n.hash.clone()));
		}

		// If we already printed a message for a given block recently,
		// we should not print it again.
		// 这里主要判断是否已经包含这个块儿的打印如果已经包括那么就不需要在进行打印了
		if !last_blocks.contains(&n.hash) {
			last_blocks.push_back(n.hash.clone());

			if last_blocks.len() > max_blocks_to_track {
				last_blocks.pop_front();
			}

			info!(
				target: "substrate",
				"✨ Imported #{} ({})",
				Colour::White.bold().paint(format!("{}", n.header.number())),
				n.hash,
			);
		}

		future::ready(())
	})
}
